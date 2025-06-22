use x86_64::structures::idt::InterruptStackFrame;

// use crate::{
//     cpu_local_data::get_local,
//     memory::MEMORY,
//     run_tasks::run_threads,
//     task::{THREAD_PRIORITIES, THREADS},
// };

pub extern "x86-interrupt" fn preempt_ipi_handler(_stack_frame: InterruptStackFrame) {
    todo!()
    // {
    //     let mut thread_priorities = THREAD_PRIORITIES.write();
    //     let mut threads = THREADS.write();
    //     let local = get_local();
    //     if let Some(current_thread_id) = local.running_thread.try_lock().unwrap().as_ref() {
    //         let current_process_id = threads.get(current_thread_id).unwrap().process.id;
    //         let index = thread_priorities
    //             .iter()
    //             .position(|thread_id| thread_id == current_thread_id)
    //             .unwrap();
    //         thread_priorities.remove(index);
    //         threads.remove(current_thread_id);
    //         if threads
    //             .values()
    //             .all(|thread| thread.process.id != current_process_id)
    //         {
    //             MEMORY
    //                 .get()
    //                 .unwrap()
    //                 .physical_memory
    //                 .lock()
    //                 .remove_user_mode_memory();
    //         }
    //     }
    // }
    // run_threads()
}
