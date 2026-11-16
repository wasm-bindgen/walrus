mod dwarf;

use crate::emit::{Emit, EmitContext};
use crate::ir::{
    dfs_in_order, Instr, InstrLocId, Local, LocalGet, LocalId, LocalSet, LocalTee, Visitor,
};
use crate::map::IdHashMap;
use crate::parse::IndicesToIds;
use crate::{CustomSection, Function, FunctionId, GlobalId, Module, RawCustomSection};
use gimli::*;
use std::cell::RefCell;
use std::collections::HashMap;

use self::dwarf::{CodeMap, Indices, InputFunction, OutputFunction};

/// The DWARF debug section in input WebAssembly binary.
#[derive(Debug, Default)]
pub struct ModuleDebugData {
    /// DWARF debug data
    pub dwarf: read::Dwarf<Vec<u8>>,
    /// The input module's locals of each function and its globals, by the
    /// indices `DW_OP_WASM_location` uses. Recorded when `generate_dwarf` is
    /// on.
    input_indices: Option<InputIndices>,
}

#[derive(Debug)]
struct InputIndices {
    locals: IdHashMap<Function, Vec<LocalId>>,
    globals: Vec<GlobalId>,
    /// The function and local index of each input `local.get`, `local.set`
    /// and `local.tee`, sorted by location.
    local_operands: Vec<(InstrLocId, FunctionId, u32)>,
}

impl Module {
    pub(crate) fn parse_debug_sections(
        &mut self,
        mut debug_sections: Vec<RawCustomSection>,
        indices: &IndicesToIds,
    ) -> Result<()> {
        let has_debug_info = debug_sections
            .iter()
            .any(|s| s.name() == ".debug_info" && !s.data.is_empty());
        let load_section = |id: gimli::SectionId| -> Result<Vec<u8>> {
            Ok(
                match debug_sections
                    .iter_mut()
                    .find(|section| section.name() == id.name())
                {
                    Some(section) => std::mem::take(&mut section.data),
                    None => Vec::new(),
                },
            )
        };

        self.debug.dwarf = read::Dwarf::load(load_section)?;
        if self.config.generate_dwarf && has_debug_info {
            let (locals, globals) = indices.local_and_global_tables();
            let mut local_operands = Vec::new();
            for (id, func) in self.funcs.iter_local_mut() {
                let operands = std::mem::take(&mut func.dwarf_local_operands);
                local_operands.extend(operands.into_iter().map(|(loc, i)| (loc, id, i)));
            }
            local_operands.sort_unstable_by_key(|x| x.0);
            self.debug.input_indices = Some(InputIndices {
                locals,
                globals,
                local_operands,
            });
        }

        Ok(())
    }
}

impl Emit for ModuleDebugData {
    fn emit(&self, cx: &mut EmitContext) {
        let code_section_start = cx.code_transform.code_section_start;
        let inputs = cx
            .module
            .funcs
            .iter_local()
            .filter_map(|(id, f)| {
                let range = f.original_range.clone()?;
                Some(InputFunction {
                    id,
                    start: range.start as u64,
                    end: range.end as u64,
                    instrs: &f.instruction_mapping,
                })
            })
            .collect();
        let ranges: IdHashMap<Function, _> =
            cx.code_transform.function_ranges.iter().cloned().collect();
        let outputs = cx
            .function_instructions
            .iter()
            .filter_map(|(id, instrs)| {
                let range = ranges.get(id)?;
                Some(OutputFunction {
                    id: *id,
                    start: (range.start - code_section_start) as u64,
                    end: (range.end - code_section_start) as u64,
                    instrs,
                    base: code_section_start,
                })
            })
            .collect();
        let code = CodeMap::new(inputs, outputs);

        #[allow(deprecated)]
        let from_dwarf = cx
            .module
            .debug
            .dwarf
            .borrow(|sections| EndianRcSlice::new(sections.as_slice().into(), LittleEndian));

        // Locals and globals that walrus removed have no index.
        // A pass may replace locals (coalescing them, or giving inlined code
        // its own), so if the input local isn't emitted in the output
        // function, see which local its input instructions use there.
        let renamed: RefCell<IdHashMap<Function, LocalRenames>> = Default::default();
        let local = |input_func: FunctionId, output_func: FunctionId, index: u32| {
            let input = self.input_indices.as_ref()?;
            let id = input.locals.get(&input_func)?.get(index as usize)?;
            let emitted = cx.indices.locals.get(&output_func)?;
            if let Some(&i) = emitted.get(id) {
                return Some(i);
            }
            let mut renamed = renamed.borrow_mut();
            let renamed = renamed.entry(output_func).or_insert_with(|| {
                let func = cx.module.funcs.get(output_func).kind.unwrap_local();
                local_renames(func, &input.local_operands, emitted)
            });
            renamed.get(&(input_func, index)).copied()
        };
        let global = |index: u32| -> Option<u32> {
            let input = self.input_indices.as_ref()?;
            cx.indices
                .emitted_global_index(*input.globals.get(index as usize)?)
        };
        let indices = self.input_indices.as_ref().map(|_| Indices {
            local: &local,
            global: &global,
        });

        let mut dwarf =
            dwarf::convert(&from_dwarf, &code, indices.as_ref()).expect("cannot convert DWARF");

        let mut sections = write::Sections::new(write::EndianVec::new(gimli::LittleEndian));
        dwarf.write(&mut sections).expect("write failed");
        sections
            .for_each(
                |id: SectionId, data: &write::EndianVec<LittleEndian>| -> Result<()> {
                    if !data.slice().is_empty() {
                        cx.wasm_module.section(&wasm_encoder::CustomSection {
                            name: id.name().into(),
                            data: data.slice().into(),
                        });
                    }
                    Ok(())
                },
            )
            .expect("never");
    }
}

/// The emitted local index for an input (function, local index).
type LocalRenames = HashMap<(FunctionId, u32), u32>;

/// For each input (function, local index) whose `local.get`, `local.set` or
/// `local.tee` instructions are in `func`, the emitted index those
/// instructions use most.
fn local_renames(
    func: &crate::LocalFunction,
    input: &[(InstrLocId, FunctionId, u32)],
    emitted: &IdHashMap<Local, u32>,
) -> LocalRenames {
    struct Operands<'a>(Vec<(InstrLocId, &'a LocalId)>);
    impl<'instr> Visitor<'instr> for Operands<'instr> {
        fn visit_instr(&mut self, instr: &'instr Instr, loc: &'instr InstrLocId) {
            match instr {
                Instr::LocalGet(LocalGet { local })
                | Instr::LocalSet(LocalSet { local })
                | Instr::LocalTee(LocalTee { local }) => self.0.push((*loc, local)),
                _ => {}
            }
        }
    }
    let mut operands = Operands(Vec::new());
    dfs_in_order(&mut operands, func, func.entry_block());
    let mut votes: HashMap<(FunctionId, u32), HashMap<u32, usize>> = HashMap::new();
    for (loc, local) in operands.0 {
        let Ok(i) = input.binary_search_by_key(&loc, |x| x.0) else {
            continue;
        };
        let (_, input_func, index) = input[i];
        if let Some(&new) = emitted.get(local) {
            *votes
                .entry((input_func, index))
                .or_default()
                .entry(new)
                .or_default() += 1;
        }
    }
    votes
        .into_iter()
        .filter_map(|(k, v)| {
            // Most votes, then the lowest index, so the result is stable.
            let (new, _) = v
                .into_iter()
                .max_by_key(|&(new, n)| (n, std::cmp::Reverse(new)))?;
            Some((k, new))
        })
        .collect()
}
