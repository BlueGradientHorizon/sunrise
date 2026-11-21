//! Calls reached by entering the checked function or callback, not merely written inside it.
use super::{Build, source};
use mod_source::index::{Found, Index, ItemId};
use std::{collections::HashSet, fmt};
use syn::{Block, Expr, FnArg, Item, ItemFn, Pat, Stmt, visit::Visit};

pub(crate) enum Callee<'ast> {
    Method(&'ast syn::Ident),
    Function(&'ast syn::Path),
}

impl<'ast> Callee<'ast> {
    pub(super) fn from_expr(expr: &'ast Expr) -> Option<Self> {
        match expr {
            Expr::MethodCall(call) => Some(Self::Method(&call.method)),
            Expr::Call(call) => Some(Self::Function(function_path(&call.func)?)),
            _ => None,
        }
    }
}

pub(super) fn ungroup(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(paren) => ungroup(&paren.expr),
        Expr::Group(group) => ungroup(&group.expr),
        _ => expr,
    }
}

pub(super) fn function_path(expr: &Expr) -> Option<&syn::Path> {
    match ungroup(expr) {
        Expr::Path(path) if path.qself.is_none() => Some(&path.path),
        _ => None,
    }
}

pub(super) fn path_is(path: &syn::Path, expected: &str) -> bool {
    let mut names = expected.split("::");
    path.segments
        .iter()
        .all(|segment| names.next().is_some_and(|name| segment.ident == name))
        && names.next().is_none()
}

/// Authoring's placeholder check only detects whether a call is already written. It does not
/// certify a production route; the independent wiring check below does that.
pub(crate) fn has_call_matching<'ast>(
    block: &'ast Block,
    matches: impl Fn(Callee<'ast>) -> bool,
) -> bool {
    let build = Build::default();
    let mut calls = Calls::new(&build);
    calls.syntax_only = true;
    calls
        .in_block(block)
        .into_iter()
        .filter_map(Callee::from_expr)
        .any(matches)
}

struct Scope<'ast> {
    functions: Vec<&'ast ItemFn>,
    locals: Vec<(&'ast syn::Ident, Option<&'ast Expr>)>,
}

/// The calls in one active cfg assignment. Closures (including stored local closures) each
/// start a new body, entered only when invoked or when passed to a helper that invokes them.
pub(super) struct Calls<'b, 'ast, F> {
    build: &'b Build,
    runs_arg: F,
    scopes: Vec<Scope<'ast>>,
    active_closures: Vec<&'ast Expr>,
    found: Vec<&'ast Expr>,
    syntax_only: bool,
}

impl<'b, 'ast> Calls<'b, 'ast, fn(&syn::Path, usize) -> bool> {
    pub(super) fn new(build: &'b Build) -> Self {
        Self::with_helpers(build, |_, _| false)
    }
}

impl<'b, 'ast, F: FnMut(&syn::Path, usize) -> bool> Calls<'b, 'ast, F> {
    fn with_helpers(build: &'b Build, runs_arg: F) -> Self {
        Self {
            build,
            runs_arg,
            scopes: Vec::new(),
            active_closures: Vec::new(),
            found: Vec::new(),
            syntax_only: false,
        }
    }

    pub(super) fn in_block(mut self, block: &'ast Block) -> Vec<&'ast Expr> {
        self.visit_block(block);
        self.found
    }

    fn in_expr(mut self, expression: &'ast Expr) -> Vec<&'ast Expr> {
        self.visit_expr(expression);
        self.found
    }

    fn local(&self, path: &syn::Path) -> Option<Option<&'ast Expr>> {
        let name = path.get_ident()?;
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.locals.iter().rev())
            .find_map(|(local, init)| (*local == name).then_some(*init))
    }

    fn closure(&self, expression: &'ast Expr) -> Option<&'ast Expr> {
        let expression = ungroup(expression);
        let resolved = if let Some(path) = function_path(expression) {
            ungroup(self.local(path)??)
        } else {
            expression
        };
        matches!(resolved, Expr::Closure(_)).then_some(resolved)
    }

    fn invoke(&mut self, expression: &'ast Expr) {
        if let Expr::Closure(closure) = expression
            && self.build.enabled(&closure.attrs)
            && !self
                .active_closures
                .iter()
                .any(|active| std::ptr::eq(*active, expression))
        {
            self.active_closures.push(expression);
            self.visit_expr(&closure.body);
            self.active_closures.pop();
        }
    }

    fn helper_runs(&mut self, path: &syn::Path, argument: usize) -> bool {
        // The engine's collision adapter uses crash::contain, which runs the closure under
        // catch_unwind. That standard-library edge has no function body in the source index.
        if path_is(path, "sunrise_client::crash::contain") {
            return argument == 0;
        }
        if let Some(name) = path.get_ident()
            && let Some(helper) = self
                .scopes
                .iter()
                .rev()
                .flat_map(|scope| &scope.functions)
                .find(|helper| helper.sig.ident == *name)
        {
            return parameter_called(self.build, helper, argument);
        }
        (self.runs_arg)(path, argument)
    }
}

impl<'ast, F: FnMut(&syn::Path, usize) -> bool> Visit<'ast> for Calls<'_, 'ast, F> {
    fn visit_block(&mut self, block: &'ast Block) {
        let functions = block
            .stmts
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::Item(Item::Fn(item)) if self.build.enabled(&item.attrs) => Some(item),
                _ => None,
            })
            .collect();
        self.scopes.push(Scope {
            functions,
            locals: Vec::new(),
        });
        syn::visit::visit_block(self, block);
        self.scopes.pop();
    }

    fn visit_item(&mut self, _: &'ast Item) {}

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if self.build.enabled(&local.attrs) {
            syn::visit::visit_local(self, local);
            if let Pat::Ident(pat) = &local.pat
                && let Some(scope) = self.scopes.last_mut()
            {
                scope.locals.push((
                    &pat.ident,
                    local.init.as_ref().map(|init| init.expr.as_ref()),
                ));
            }
        }
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if self.build.enabled(&arm.attrs) {
            syn::visit::visit_arm(self, arm);
        }
    }

    fn visit_field_value(&mut self, field: &'ast syn::FieldValue) {
        if self.build.enabled(&field.attrs) {
            syn::visit::visit_field_value(self, field);
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if !self.build.enabled(source::expr_attrs(expr)) {
            return;
        }
        if self.syntax_only {
            if matches!(expr, Expr::Call(_) | Expr::MethodCall(_)) {
                self.found.push(expr);
            }
        } else {
            match expr {
                Expr::Closure(_) | Expr::Async(_) => return,
                Expr::Call(call) => {
                    let path = function_path(&call.func);
                    if path.is_none_or(|path| self.local(path).is_none()) {
                        self.found.push(expr);
                    }
                    if let Some(closure) = self.closure(&call.func) {
                        self.invoke(closure);
                    }
                    if let Some(path) = path
                        && self.local(path).is_none()
                    {
                        for (index, argument) in call.args.iter().enumerate() {
                            if let Some(closure) = self.closure(argument)
                                && self.helper_runs(path, index)
                            {
                                self.invoke(closure);
                            }
                        }
                    }
                }
                Expr::MethodCall(call) => {
                    self.found.push(expr);
                    // LevelControl::hear invokes the dispatch with the current control marked.
                    if call.method == "hear"
                        && let Some(receiver) = function_path(&call.receiver)
                        && receiver.segments.len() == 5
                        && receiver
                            .segments
                            .iter()
                            .take(4)
                            .map(|segment| &segment.ident)
                            .zip(["crate", "game", "level_control_hooks", "LevelControl"])
                            .all(|(ident, segment)| ident == segment)
                        && let Some(closure) = call.args.first().and_then(|arg| self.closure(arg))
                    {
                        self.invoke(closure);
                    }
                    // The effect dispatcher filters its parts by cadence. Creating that iterator
                    // does not ask the parts; advancing it runs the predicate.
                    if call.method == "next" {
                        let receiver = function_path(&call.receiver)
                            .and_then(|path| self.local(path).flatten())
                            .unwrap_or(&call.receiver);
                        if let Expr::MethodCall(filter) = ungroup(receiver)
                            && filter.method == "filter"
                            && let Expr::Call(parts) = ungroup(&filter.receiver)
                            && function_path(&parts.func)
                                .is_some_and(|path| path_is(path, "crate::entered_parts"))
                            && let Some(closure) =
                                filter.args.first().and_then(|arg| self.closure(arg))
                        {
                            self.invoke(closure);
                        }
                    }
                }
                _ => {}
            }
        }
        syn::visit::visit_expr(self, expr);
    }
}

fn parameter_called(build: &Build, helper: &ItemFn, argument: usize) -> bool {
    let Some(FnArg::Typed(arg)) = helper.sig.inputs.iter().nth(argument) else {
        return false;
    };
    let Pat::Ident(pat) = arg.pat.as_ref() else {
        return false;
    };
    Calls::new(build).in_block(&helper.block).iter().any(|expr| {
        matches!(Callee::from_expr(expr), Some(Callee::Function(path)) if path.is_ident(&pat.ident))
    })
}

#[derive(Clone, Copy)]
pub(super) enum Target<'a> {
    Path(&'a str),
    Definition {
        package: &'a str,
        module: &'a str,
        name: &'a str,
    },
}

impl fmt::Display for Target<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(path) => f.write_str(path),
            Self::Definition {
                package,
                module,
                name,
            } => write!(f, "{package}::{module}::{name}"),
        }
    }
}

/// Follow the functions actually called by an installed callback through the shared resolver.
pub(super) struct Routes<'b, 'i, 's> {
    index: &'i mut Index<'s>,
    build: &'b Build,
}

impl<'b, 'i, 's> Routes<'b, 'i, 's> {
    pub(super) fn new(index: &'i mut Index<'s>, build: &'b Build) -> Self {
        Self { index, build }
    }

    pub(super) fn calls<'r, 'ast>(
        &'r mut self,
        scope: usize,
    ) -> Calls<'r, 'ast, impl FnMut(&syn::Path, usize) -> bool + 'r> {
        let build = self.build;
        Calls::with_helpers(build, move |path, argument| {
            if let Some(Found::Item(id)) = self.resolve(scope, path)
                && let Item::Fn(helper) = self.index.item(id)
            {
                parameter_called(build, helper, argument)
            } else {
                false
            }
        })
    }

    pub(super) fn reaches(&mut self, scope: usize, callback: &Expr, target: Target<'_>) -> bool {
        let mut seen = HashSet::new();
        match ungroup(callback) {
            Expr::Path(path) if path.qself.is_none() => {
                self.follows(scope, &path.path, target, &mut seen)
            }
            Expr::Closure(callback) if self.build.enabled(&callback.attrs) => {
                let calls = self.calls(scope).in_expr(&callback.body);
                self.follow_calls(scope, &calls, target, &mut seen)
            }
            _ => false,
        }
    }

    fn follow_calls(
        &mut self,
        scope: usize,
        calls: &[&Expr],
        target: Target<'_>,
        seen: &mut HashSet<ItemId>,
    ) -> bool {
        let mut paths = calls
            .iter()
            .filter_map(|expr| match Callee::from_expr(expr) {
                Some(Callee::Function(path)) => Some(path),
                _ => None,
            });
        if let Target::Path(expected) = target
            && paths.clone().any(|path| path_is(path, expected))
        {
            return true;
        }
        paths.any(|path| self.follows(scope, path, target, seen))
    }

    fn follows(
        &mut self,
        scope: usize,
        path: &syn::Path,
        target: Target<'_>,
        seen: &mut HashSet<ItemId>,
    ) -> bool {
        if matches!(target, Target::Path(expected) if path_is(path, expected)) {
            return true;
        }
        match self.resolve(scope, path) {
            Some(Found::Item(id)) => {
                let Item::Fn(callback) = self.index.item(id) else {
                    return false;
                };
                let module = &self.index.modules[id.module];
                if let Target::Definition {
                    package,
                    module: expected,
                    name,
                } = target
                    && module.krate == package
                    && callback.sig.ident == name
                    && module
                        .path
                        .iter()
                        .map(String::as_str)
                        .eq(expected.split("::"))
                {
                    return true;
                }
                if !seen.insert(id) {
                    return false;
                }
                let body = callback.block.clone();
                let calls = self.calls(id.module).in_block(&body);
                self.follow_calls(id.module, &calls, target, seen)
            }
            Some(Found::External(path)) => matches!(target, Target::Path(expected)
                if path.iter().map(String::as_str).eq(expected.split("::"))),
            None => false,
        }
    }

    fn resolve(&mut self, scope: usize, path: &syn::Path) -> Option<Found> {
        let path: Vec<_> = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect();
        self.index.resolve_path(scope, &path).ok()
    }
}
