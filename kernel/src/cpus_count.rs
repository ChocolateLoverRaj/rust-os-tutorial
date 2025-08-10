use crate::limine_requests::MP_REQUEST;

pub fn cpus_count() -> usize {
    MP_REQUEST.get_response().unwrap().cpus().len()
}
