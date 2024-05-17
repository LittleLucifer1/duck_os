
#[macro_export]
macro_rules! stack_trace {
    () => {
        file!();
        line!();
    };
}