//! A command's options, read once by the code that uses them: whatever the command does not read
//! is refused by name in [`Options::finish`], so a mistyped option is never silently ignored.

/// The arguments of one command (after its name), with what has been read from them so far.
pub struct Options<'a> {
    command: &'a str,
    args: &'a [String],
    read: Vec<bool>,
    /// Where a bare `--` begins what the command hands on untouched (`run`'s game arguments).
    end: usize,
}

impl<'a> Options<'a> {
    /// `command` is the name the refusal uses (`registry list`); its first word names the usage.
    pub fn new(command: &'a str, args: &'a [String]) -> Self {
        Self {
            command,
            args,
            read: vec![false; args.len()],
            end: args.iter().position(|a| a == "--").unwrap_or(args.len()),
        }
    }

    /// Whether the option `name` stands among the arguments.
    pub fn switch(&mut self, name: &str) -> bool {
        self.take(name).count() > 0
    }

    /// The value after `name` (the first one, if it is given again); it may begin with a dash.
    /// Refuses `name` as the last argument, which has no value.
    pub fn value(&mut self, name: &str) -> Result<Option<&'a str>, String> {
        Ok(self.values(name)?.first().copied())
    }

    /// The value after each `name`, in order.
    pub fn values(&mut self, name: &str) -> Result<Vec<&'a str>, String> {
        let mut out = Vec::new();
        for i in self.take(name).collect::<Vec<_>>() {
            if i + 1 >= self.end {
                return Err(self.refusal(&format!("{name} needs a value")));
            }
            self.read[i + 1] = true;
            out.push(self.args[i + 1].as_str());
        }
        Ok(out)
    }

    /// The arguments that are not options, in order. Read the options first: a value is
    /// positional until its option has taken it.
    pub fn positional(&mut self) -> Vec<&'a str> {
        let mut out = Vec::new();
        for i in 0..self.end {
            if !self.read[i] && !is_option(&self.args[i]) {
                self.read[i] = true;
                out.push(self.args[i].as_str());
            }
        }
        out
    }

    /// What follows a bare `--`.
    pub fn rest(&self) -> &'a [String] {
        self.args.get(self.end + 1..).unwrap_or_default()
    }

    /// Refuses the first option nothing read, naming it and showing the command's usage.
    pub fn finish(self) -> Result<(), String> {
        match (0..self.end).find(|&i| !self.read[i] && is_option(&self.args[i])) {
            Some(i) => {
                Err(self.refusal(&format!("{} does not take {}", self.command, self.args[i])))
            }
            None => Ok(()),
        }
    }

    fn take<'s>(&'s mut self, name: &'s str) -> impl Iterator<Item = usize> + 's {
        (0..self.end).filter(move |&i| {
            let hit = self.args[i] == name;
            self.read[i] |= hit;
            hit
        })
    }

    fn refusal(&self, what: &str) -> String {
        let usage = self
            .command
            .split(' ')
            .next()
            .and_then(crate::help::of)
            .unwrap_or_default();
        format!("{what}\n{}", usage.trim_end())
    }
}

fn is_option(arg: &str) -> bool {
    arg.len() > 1 && arg.starts_with('-')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(line: &str) -> Vec<String> {
        line.split(' ')
            .filter(|w| !w.is_empty())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn what_no_one_read_is_refused_by_name_with_the_usage() {
        let args = argv("--game g --typo extra");
        let mut options = Options::new("run", &args);
        assert_eq!(options.value("--game").unwrap(), Some("g"));
        let error = options.finish().unwrap_err();
        assert!(error.contains("--typo"), "{error}");
        assert!(error.contains("usage: cargo sunrise-mod run"), "{error}");
    }

    #[test]
    fn values_may_begin_with_a_dash_and_stop_at_a_bare_double_dash() {
        let args = argv("--name -Garden pos --jars j -- --typo --game");
        let mut options = Options::new("run", &args);
        assert_eq!(options.value("--name").unwrap(), Some("-Garden"));
        assert_eq!(options.value("--jars").unwrap(), Some("j"));
        assert_eq!(options.positional(), ["pos"]);
        assert_eq!(options.rest(), ["--typo", "--game"]);
        options.finish().unwrap();
    }

    #[test]
    fn repeated_options_are_all_read_and_a_missing_value_is_refused() {
        let args = argv("--build a --build b --all");
        let mut options = Options::new("fits", &args);
        assert_eq!(options.values("--build").unwrap(), ["a", "b"]);
        assert!(options.switch("--all"));
        options.finish().unwrap();

        let last = argv("--build");
        let error = Options::new("fits", &last).value("--build").unwrap_err();
        assert!(error.contains("--build"), "{error}");
    }
}
