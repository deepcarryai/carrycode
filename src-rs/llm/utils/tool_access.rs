use std::cell::Cell;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolAccessLevel {
    Workspace,
    Full,
}

thread_local! {
    static TOOL_ACCESS_LEVEL: Cell<ToolAccessLevel> = const { Cell::new(ToolAccessLevel::Workspace) };
}

pub struct ToolAccessGuard {
    prev: ToolAccessLevel,
}

impl Drop for ToolAccessGuard {
    fn drop(&mut self) {
        TOOL_ACCESS_LEVEL.with(|c| c.set(self.prev));
    }
}

pub fn with_tool_access<R>(level: ToolAccessLevel, f: impl FnOnce() -> R) -> R {
    let prev = TOOL_ACCESS_LEVEL.with(|c| {
        let prev = c.get();
        c.set(level);
        prev
    });
    let _guard = ToolAccessGuard { prev };
    f()
}

pub fn current_tool_access() -> ToolAccessLevel {
    TOOL_ACCESS_LEVEL.with(|c| c.get())
}

pub fn is_full_access() -> bool {
    current_tool_access() == ToolAccessLevel::Full
}

