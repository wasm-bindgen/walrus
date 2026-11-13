mod dwarf;
mod expression;
mod rust_names;
mod scopes;
mod source_map;

use crate::emit::EmitContext;
use crate::map::IdHashMap;
use crate::parse::IndicesToIds;
use crate::{CustomSection, Function, FunctionId, GlobalId, LocalId, Module, RawCustomSection};
use gimli::*;

use self::dwarf::{Addresses, Indices};
use self::expression::{CodeAddressConverter, CodeAddressGenerator};

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
            self.debug.input_indices = Some(InputIndices { locals, globals });
        }

        Ok(())
    }
}

impl ModuleDebugData {
    /// Rewrites the DWARF for the emitted code, and writes it to the module
    /// when `generate_dwarf` is set. With `source_map`, returns the source map
    /// built from the rewritten DWARF.
    pub(crate) fn emit(&self, cx: &mut EmitContext, source_map: bool) -> Option<String> {
        let address_generator = CodeAddressGenerator::new(&cx.module.funcs);
        let address_converter = CodeAddressConverter::new(&cx.code_transform);
        let code_section_start = cx.code_transform.code_section_start;

        let map = |address: u64, search_preference| -> Option<u64> {
            let code = address_generator.find_address(address as usize, search_preference);
            address_converter
                .find_address(code)
                .map(|x| (x - code_section_start) as u64)
        };
        let mut input_bodies = cx
            .module
            .funcs
            .iter_local()
            .filter_map(|(id, f)| Some((id, f.original_range.clone()?)))
            .map(|(id, r)| (r.start as u64, r.end as u64, id))
            .collect::<Vec<_>>();
        input_bodies.sort_unstable_by_key(|&(start, _, _)| start);
        let mut output_bodies = cx
            .code_transform
            .function_ranges
            .iter()
            .map(|(_, r)| {
                (
                    (r.start - code_section_start) as u64,
                    (r.end - code_section_start) as u64,
                )
            })
            .collect::<Vec<_>>();
        output_bodies.sort_unstable();
        let addresses = Addresses {
            map: &map,
            input_bodies,
            output_bodies,
        };

        #[allow(deprecated)]
        let from_dwarf = cx
            .module
            .debug
            .dwarf
            .borrow(|sections| EndianRcSlice::new(sections.as_slice().into(), LittleEndian));

        // Locals and globals that walrus removed have no index.
        let local = |func: FunctionId, index: u32| -> Option<u32> {
            let input = self.input_indices.as_ref()?;
            let id = input.locals.get(&func)?.get(index as usize)?;
            cx.indices.locals.get(&func)?.get(id).copied()
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

        let mut dwarf = dwarf::convert(&from_dwarf, &addresses, indices.as_ref())
            .expect("cannot convert DWARF");

        let mut sections = write::Sections::new(write::EndianVec::new(gimli::LittleEndian));
        dwarf.write(&mut sections).expect("write failed");
        if cx.module.config.generate_dwarf {
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
        let scopes = !cx.module.config.skip_source_map_scopes;
        source_map.then(|| {
            source_map::build(
                &sections,
                &cx.code_transform,
                &rust_names::RustNames,
                scopes,
            )
        })
    }
}
