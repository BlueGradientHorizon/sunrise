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
