pub struct ConfigurationTaskRegistration {
    _tasks: Vec<Box<dyn Send>>,
}

impl ConfigurationTaskRegistration {
    pub(crate) fn checkpoint(&self) -> (usize, bool) {
        (self._tasks.len(), false)
    }

    pub(crate) fn rollback(&mut self, (len, _): (usize, bool)) {
        self._tasks.truncate(len);
    }
}
