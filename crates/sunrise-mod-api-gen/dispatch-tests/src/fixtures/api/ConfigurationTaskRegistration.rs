pub struct ConfigurationTaskRegistration {
    tasks: Vec<Box<dyn Send>>,
}

impl ConfigurationTaskRegistration {
    pub fn checkpoint(&self) -> usize {
        self.tasks.len()
    }

    pub fn rollback(&mut self, len: usize) {
        self.tasks.truncate(len);
    }
}

impl ConfigurationTaskRegistration {
    pub(crate) fn checkpoint(&self) -> (usize, bool) {
        (self._tasks.len(), false)
    }

    pub(crate) fn rollback(&mut self, (len, _): (usize, bool)) {
        self._tasks.truncate(len);
    }
}
