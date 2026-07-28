#[derive(Clone, Copy, Debug)]
pub(crate) struct Logger {
    verbose: bool,
}

impl Logger {
    pub(crate) fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    pub(crate) fn log(self, message: impl AsRef<str>) {
        if self.verbose {
            eprintln!("{}", message.as_ref());
        }
    }
}
