mod barrier;
mod boot;
mod consts;
mod context;
mod gic;

#[cfg(feature = "kcontext")]
mod kcontext;
mod page_table;
mod pl011;
mod psci;
mod timer;
mod trap;

use core::slice;

use aarch64_cpu::registers::CPACR_EL1;
use aarch64_cpu::registers::{Readable, Writeable, MPIDR_EL1, TTBR0_EL1};
use alloc::vec::Vec;
pub use consts::*;
pub use context::TrapFrame;
use fdt::Fdt;

#[cfg(feature = "kcontext")]
pub use kcontext::{context_switch, context_switch_pt, read_current_tp, KContext};

pub use page_table::*;
pub use psci::system_off as shutdown;
pub use trap::run_user_task;

use crate::MultiCore;
use crate::utils::init_once::InitOnce;
use crate::PageTable;
use super::{clear_bss, CPU_NUM, MEM_AREA, DTB_BIN};

static DTB_PTR: InitOnce<usize> = InitOnce::new();

pub fn rust_tmp_main(hart_id: usize, device_tree: usize) {
    clear_bss();
    pl011::init_early();
    trap::init();
    gic::init();

    timer::init();

    DTB_PTR.init_once_by(device_tree | VIRT_ADDR_START);

    if let Ok(fdt) = unsafe { Fdt::from_ptr(*DTB_PTR as *const u8) } {
        CPU_NUM.init_by(fdt.cpus().count());
    } else {
        CPU_NUM.init_by(1);
    }

    // Enable Floating Point Feature.
    CPACR_EL1.write(CPACR_EL1::FPEN::TrapNothing);
    aarch64_cpu::asm::barrier::isb(aarch64_cpu::asm::barrier::SY);

    // Enter to kernel entry point(`main` function).
    unsafe { crate::_main_for_arch(hart_id) };

    shutdown();
}

pub fn kernel_page_table() -> PageTable {
    PageTable(crate::PhysAddr(TTBR0_EL1.get_baddr() as _))
}

#[inline]
pub fn hart_id() -> usize {
    MPIDR_EL1.read(MPIDR_EL1::Aff0) as _
}

pub(crate) fn arch_init() {
    let mut buffer = Vec::new();
    if let Ok(fdt) = unsafe { Fdt::from_ptr(*DTB_PTR as *const u8) } {
        unsafe {
            buffer.extend_from_slice(slice::from_raw_parts(
                *DTB_PTR as *const u8,
                fdt.total_size(),
            ));
        }
    }
    DTB_BIN.init_by(buffer);
    if let Ok(fdt) = Fdt::new(&DTB_BIN) {
        info!("There has {} CPU(s)", fdt.cpus().count());
        let mut mem_area = Vec::new();
        fdt.memory().regions().for_each(|x| {
            info!(
                "memory region {:#X} - {:#X}",
                x.starting_address as usize,
                x.starting_address as usize + x.size.unwrap()
            );
            mem_area.push((
                x.starting_address as usize | VIRT_ADDR_START,
                x.size.unwrap_or(0),
            ));
        });
        MEM_AREA.init_by(mem_area);
    }
}

#[cfg(feature = "multicore")]
impl MultiCore {
    /// Boot application cores
    pub fn boot_all() {}
}
