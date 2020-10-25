use rustyl4api::object::{VTableObj, RamObj, CNodeObj, TcbObj, EpCap, TcbCap, UntypedObj};
use rustyl4api::vspace::Permission;
use spaceman::vspace_man::VSpaceMan;

#[derive(Debug)]
pub struct ProcessBuilder<'a> {
    elf: &'a [u8],
    stdio: Option<EpCap>,
}

pub struct Child {
    vspace: VSpaceMan,
    tcb: TcbCap,
    stdio: Option<EpCap>,
}

impl<'a> ProcessBuilder<'a> {
    pub fn new(elf: &'a [u8]) -> Self {
        Self {
            elf: elf,
            stdio: None,
        }
    }

    pub fn stdio(mut self, ep: EpCap) -> Self {
        self.stdio = Some(ep);
        self
    }

    pub fn spawn(self) -> Result<Child, ()> {
        use rustyl4api::object::cnode::{CNODE_ENTRY_SZ};
        use rustyl4api::object::tcb::TCB_OBJ_BIT_SZ;
        use rustyl4api::vspace::{FRAME_BIT_SIZE, FRAME_SIZE};
        use rustyl4api::process::{ProcessCSpace, PROCESS_ROOT_CNODE_SIZE};
        use crate::space_manager::gsm;


        let rootcn_bitsz = (PROCESS_ROOT_CNODE_SIZE * CNODE_ENTRY_SZ).trailing_zeros() as usize;
        let child_tcb = gsm!().alloc_object::<TcbObj>(TCB_OBJ_BIT_SZ).unwrap();
        let child_root_cn = gsm!().alloc_object::<CNodeObj>(rootcn_bitsz).unwrap();
        let child_root_vn = gsm!().alloc_object::<VTableObj>(12).unwrap();
        let vspace = VSpaceMan::new(child_root_vn.clone());

        let mut cur_free = ProcessCSpace::ProcessFixedMax as usize;

        let entry = elfloader::load_elf(self.elf, 0x8000000, 4096, &mut |vrange, flags| {
            use rustyl4api::object::RamCap;

            let vaddr = vrange.start as usize;
            let perm = Permission::new (
                flags & 0b100 != 0,
                flags & 0b010 != 0,
                flags & 0b001 != 0,
            );
            let frame_cap = gsm!().alloc_object::<RamObj>(FRAME_BIT_SIZE).unwrap();
            let frame_parent_slot = gsm!().cspace_alloc().unwrap();
            frame_cap.derive(frame_parent_slot).unwrap();
            let frame_parent_cap = RamCap::new(frame_parent_slot);

            while let Err(e) = vspace.map_frame(frame_cap.clone(), vaddr, perm, 4) {
                match e {
                    // VSpaceManError::SlotOccupied{level} => {
                    //     panic!("slot occupied at level {} vaddr {:x}", level, vaddr);
                    // }
                    // VSpaceManError::SlotTypeError{level} => {
                    //     panic!("wrong slot type at level {} vaddr {:x}", level, vaddr);
                    // }
                    // VSpaceManError::PageTableMiss{level} => {
                    rustyl4api::error::SysError::VSpaceTableMiss{level} => {
                        let vtable_cap = gsm!().alloc_object::<VTableObj>(12).unwrap();
                        // kprintln!("miss table level {} addr {:x}", level, vaddr);
                        vspace.map_table(vtable_cap.clone(), vaddr, level as usize).unwrap();
                        child_root_cn.cap_copy(cur_free, vtable_cap.slot).map_err(|_| ()).unwrap();
                        cur_free += 1;
                    }
                    e => {
                        panic!("vaddr {:x} perm {:?} error: {:?}", vaddr, perm, e);
                    }
                }
            };

            child_root_cn.cap_copy(cur_free, frame_cap.slot).map_err(|_| ()).unwrap();
            cur_free += 1;
            let frame_addr = gsm!().insert_ram_at(frame_parent_cap.clone(), 0, Permission::writable());
            let frame = unsafe {
                core::slice::from_raw_parts_mut(frame_addr, FRAME_SIZE)
            };
            frame
        }).map_err(|_| ())?;

        child_tcb.configure(Some(child_root_vn.slot), Some(child_root_cn.slot))
            .expect("Error Configuring TCB");
        child_tcb.set_registers(0b1100, entry as usize, 0x8000000)
            .expect("Error Setting Registers");
        child_root_cn.cap_copy(ProcessCSpace::TcbCap as usize, child_tcb.slot).map_err(|_| ())?;
        child_root_cn.cap_copy(ProcessCSpace::RootCNodeCap as usize, child_root_cn.slot).map_err(|_| ())?;
        child_root_cn.cap_copy(ProcessCSpace::RootVNodeCap as usize, child_root_vn.slot).map_err(|_| ())?;
        if let Some(ep) = &self.stdio {
            child_root_cn.cap_copy(ProcessCSpace::Stdio as usize, ep.slot).map_err(|_| ())?;
        }
        let init_untyped = gsm!().alloc_object::<UntypedObj>(16).ok_or(())?;
        child_root_cn.cap_copy(ProcessCSpace::InitUntyped as usize, init_untyped.slot).map_err(|_| ())?;

        child_tcb.resume()
            .expect("Error Resuming TCB");

        Ok(Child {
            vspace: vspace,
            tcb: child_tcb,
            stdio: self.stdio,
        })
    }
}