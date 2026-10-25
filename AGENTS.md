# Avoid
- over explaining and long prose anywhere be it either code or git comments or readmes. Keep things succint and in a plain, easy to read language.

# Always do
- ignore human estimates and present a plan on how you're going to solve the given problem, explain tradeoffs 
- present the scope after the plan is agreed
- always work within the boundaries of the approved scope, never more, never try to be overly helpful

# Definiton of done
- `pnpm lint` passes
- `cargo clippy` and `ast-grep` checks pass
- Run adverserial review and fix all issues pertaining to the change you worked on. 
- Repeat lint, clippy, ast-grep and adverserial review until all green.
- Wait for review and approval before commit.
