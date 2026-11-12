mod dwarf;
mod expression;

use crate::emit::{Emit, EmitContext};
use crate::{CustomSection, Module, RawCustomSection};
use gimli::*;

use self::dwarf::Addresses;
use self::expression::{CodeAddressConverter, CodeAddressGenerator};

/// The DWARF debug section in input WebAssembly binary.
#[derive(Debug, Default)]
pub struct ModuleDebugData {
    /// DWARF debug data
    pub dwarf: read::Dwarf<Vec<u8>>,
}

impl Module {
    pub(crate) fn parse_debug_sections(
        &mut self,
        mut debug_sections: Vec<RawCustomSection>,
    ) -> Result<()> {
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

        Ok(())
    }
}

impl Emit for ModuleDebugData {
    fn emit(&self, cx: &mut EmitContext) {
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
            .filter_map(|(_, f)| f.original_range.clone())
            .map(|r| (r.start as u64, r.end as u64))
            .collect::<Vec<_>>();
        input_bodies.sort_unstable();
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
            .borrow(|sections| EndianSlice::new(sections.as_ref(), LittleEndian));

        let mut dwarf = dwarf::convert(&from_dwarf, &addresses).expect("cannot convert DWARF");

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
