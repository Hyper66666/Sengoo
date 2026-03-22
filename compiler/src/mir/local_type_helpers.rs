use crate::mir::{Local, MIRType};

pub(crate) fn collect_local_types<F>(locals: &[Local], mut get_local_type: F) -> Vec<MIRType>
where
    F: FnMut(Local) -> MIRType,
{
    locals.iter().map(|local| get_local_type(*local)).collect()
}
