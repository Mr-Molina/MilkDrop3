### Task 1.4: Bytecode compiler

**Files:**
- Create: `crates/md-eval/src/bytecode.rs`, `crates/md-eval/src/compiler.rs`

**Interfaces:**
- Produces: `pub enum Op { PushConst(f64), Load(VarRef), Store(VarRef), LoadMem(bool /* global */), StoreMem(bool), Pop, Dup, Add, Sub, Mul, Div, Mod, Pow, BitAnd, BitOr, Eq, Ne, Lt, Gt, Le, Ge, And, Or, Not, Neg, Call1(Fn1), Call2(Fn2), Jump(u32), JumpIfFalse(u32), Loop { body_start: u32, body_end: u32 }, While { body_start: u32, body_end: u32 } }`; `pub fn compile_ast(vm: &mut Vm, ast: &Expr) -> Result<Program, CompileError>`; `Program::compile` from A.3 composes lexer, parser, and this.
- `_if(c,a,b)`, `&&`, `||`, and `? :` compile to jumps so the untaken branch is not evaluated (ns-eel2 evaluates `_if` lazily; `_and`/`_or` likewise).
- Integer semantics: `%`, `&`, `|` convert both operands with truncation toward zero to `i32`; `%` by zero yields 0.

- [ ] **Step 1:** Write failing tests `if_does_not_evaluate_untaken_branch` (`if(0, a=1, b=1)` leaves `a` at 0), `mod_by_zero_is_zero`, `bitops_truncate_operands` (`7.9 & 3.2` == 3), `loop_count_is_floored_and_capped`.
- [ ] **Step 2:** Run `cargo test -p md-eval compiler`. Expected: FAIL.
- [ ] **Step 3:** Implement the compiler and a stack VM `Program::run` in `vm.rs`.
- [ ] **Step 4:** Run the tests. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(eval): bytecode compiler and VM`

