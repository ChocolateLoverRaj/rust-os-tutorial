use crate::{
    cpu_local_data::get_local,
    guarded_stack::{GuardedStack, StackType},
    local_apic_id::LocalApicId,
};

pub fn create_normal_stack(f: extern "sysv64" fn() -> !) -> ! {
    let local = get_local();
    let stack = GuardedStack::new(64 * 0x400, StackType::Normal(LocalApicId::from(local.cpu)));
    local.normal_stack.call_once(|| stack.top());
    stack.switch(f)
}
