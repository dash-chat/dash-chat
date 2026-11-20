#![feature(rustc_private)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::path::PathBuf;

use clippy_utils::diagnostics::span_lint_and_then;
use clippy_utils::{get_async_fn_body, is_in_test};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Body, Expr, ExprKind, FnDecl, LoopSource, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::hir::nested_filter::OnlyBodies;
use rustc_middle::ty::TyCtxt;
use rustc_span::{FileName, Span};

const MIN_DELEGATING_STATEMENTS: usize = 2;
const MAX_DELEGATING_LINES: usize = 5;
const MIN_DETAIL_LINES: usize = 8;
const MIN_DETAIL_DEPTH: u32 = 2;
const MIN_DETAIL_OPERATIONS: u32 = 4;

dylint_linting::declare_late_lint! {
    /// ### What it does
    ///
    /// Flags functions that break the Single Level of Abstraction Principle:
    /// most top-level statements delegate to other workspace functions, but
    /// at least one is a nested block of low-level work (std or third-party
    /// calls, operators, indexing, match arms).
    ///
    /// ### Why is this bad?
    ///
    /// The reader has to switch between what the function does and how one
    /// step of it is done. Extracting the block into a named helper keeps
    /// the function at one level.
    ///
    /// ### Known problems
    ///
    /// A heuristic: it reads statement shape, not intent. Bodies rewritten by
    /// attribute macros such as `#[async_trait]` are not analysed.
    ///
    /// ### Example
    ///
    /// ```rust,ignore
    /// fn sync(&self) -> Result<()> {
    ///     let peers = self.load_peers()?;
    ///     let mut by_topic = BTreeMap::new();
    ///     for peer in &peers {
    ///         if peer.last_seen > cutoff {
    ///             by_topic.entry(peer.topic).or_insert_with(Vec::new).push(peer.id);
    ///         }
    ///     }
    ///     self.publish(by_topic)
    /// }
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust,ignore
    /// fn sync(&self) -> Result<()> {
    ///     let peers = self.load_peers()?;
    ///     let by_topic = group_recent_peers_by_topic(&peers, cutoff);
    ///     self.publish(by_topic)
    /// }
    /// ```
    pub MIXED_ABSTRACTION,
    Warn,
    "function mixes delegation with inline low-level work"
}

impl<'tcx> LateLintPass<'tcx> for MixedAbstraction {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        if matches!(kind, FnKind::Closure) || span.from_expansion() {
            return;
        }
        if is_in_test(cx.tcx, cx.tcx.local_def_id_to_hir_id(def_id)) {
            return;
        }
        let value = get_async_fn_body(cx.tcx, body).unwrap_or(body.value);
        let ExprKind::Block(block, _) = value.kind else {
            return;
        };

        let mut delegating = 0;
        let mut details = Vec::new();
        let tail = block
            .expr
            .map(|expr| (expr.span, StatementShape::of_expr(cx, expr)));
        let statements = block
            .stmts
            .iter()
            .map(|stmt| (stmt.span, StatementShape::of_stmt(cx, stmt)));
        for (stmt_span, shape) in statements.chain(tail) {
            if stmt_span.from_expansion() {
                continue;
            }
            let lines = line_count(cx, stmt_span);
            if shape.is_delegation(lines) {
                delegating += 1;
            } else if shape.is_detail(lines) {
                details.push((stmt_span, shape.operations));
            }
        }
        if delegating < MIN_DELEGATING_STATEMENTS || details.is_empty() {
            return;
        }

        let operations: u32 = details.iter().map(|(_, operations)| operations).sum();
        span_lint_and_then(
            cx,
            MIXED_ABSTRACTION,
            cx.tcx.def_span(def_id),
            format!(
                "this function mixes levels of abstraction ({delegating} delegating statements, {} inline blocks, {operations} low-level operations)",
                details.len()
            ),
            |diag| {
                for (detail_span, _) in details {
                    diag.span_help(
                        detail_span,
                        "consider extracting this into a named function",
                    );
                }
            },
        );
    }
}

#[derive(Default)]
struct StatementShape {
    workspace_calls: u32,
    operations: u32,
    max_depth: u32,
}

impl StatementShape {
    fn of_stmt<'tcx>(cx: &LateContext<'tcx>, stmt: &'tcx Stmt<'tcx>) -> Self {
        let mut visitor = ShapeVisitor::new(cx);
        if !matches!(stmt.kind, StmtKind::Item(_)) {
            visitor.visit_stmt(stmt);
        }
        visitor.shape
    }

    fn of_expr<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> Self {
        let mut visitor = ShapeVisitor::new(cx);
        visitor.visit_expr(expr);
        visitor.shape
    }

    fn is_delegation(&self, lines: usize) -> bool {
        self.workspace_calls >= 1 && self.max_depth == 0 && lines <= MAX_DELEGATING_LINES
    }

    fn is_detail(&self, lines: usize) -> bool {
        self.max_depth >= MIN_DETAIL_DEPTH
            && self.operations >= MIN_DETAIL_OPERATIONS
            && self.operations >= 2 * self.workspace_calls
            && lines >= MIN_DETAIL_LINES
    }
}

struct ShapeVisitor<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    workspace_root: Option<PathBuf>,
    depth: u32,
    shape: StatementShape,
}

impl<'a, 'tcx> ShapeVisitor<'a, 'tcx> {
    fn new(cx: &'a LateContext<'tcx>) -> Self {
        Self {
            cx,
            // cargo runs rustc for workspace members from the workspace root
            workspace_root: std::env::current_dir().ok(),
            depth: 0,
            shape: StatementShape::default(),
        }
    }

    fn callee(&self, expr: &Expr<'tcx>) -> Option<DefId> {
        match expr.kind {
            ExprKind::MethodCall(..) => self.cx.typeck_results().type_dependent_def_id(expr.hir_id),
            ExprKind::Call(func, _) => {
                let ExprKind::Path(qpath) = &func.kind else {
                    return None;
                };
                match self.cx.qpath_res(qpath, func.hir_id) {
                    Res::Def(DefKind::Fn | DefKind::AssocFn, def_id) => Some(def_id),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn is_in_workspace(&self, def_id: DefId) -> bool {
        if def_id.is_local() {
            return true;
        }
        let span = self.cx.tcx.def_span(def_id);
        let FileName::Real(file) = self.cx.sess().source_map().span_to_filename(span) else {
            return false;
        };
        let (Some(path), Some(root)) = (file.local_path(), &self.workspace_root) else {
            return false;
        };
        path.is_relative() || (path.starts_with(root) && !path.starts_with(root.join("target")))
    }

    fn count(&mut self, expr: &Expr<'tcx>) {
        if expr.span.from_expansion() {
            return;
        }
        match expr.kind {
            ExprKind::Call(..) | ExprKind::MethodCall(..) => match self.callee(expr) {
                Some(def_id) if self.is_in_workspace(def_id) => self.shape.workspace_calls += 1,
                Some(_) => self.shape.operations += 1,
                None => {}
            },
            ExprKind::Binary(..) | ExprKind::AssignOp(..) | ExprKind::Index(..) => {
                self.shape.operations += 1
            }
            ExprKind::Match(_, arms, _) => self.shape.operations += arms.len() as u32,
            _ => {}
        }
    }

    fn nests(expr: &Expr<'tcx>) -> bool {
        match expr.kind {
            ExprKind::Loop(_, _, LoopSource::While | LoopSource::ForLoop, _) => true,
            ExprKind::Loop(..) | ExprKind::If(..) | ExprKind::Match(..) | ExprKind::Closure(_) => {
                !expr.span.from_expansion()
            }
            _ => false,
        }
    }
}

impl<'tcx> Visitor<'tcx> for ShapeVisitor<'_, 'tcx> {
    type NestedFilter = OnlyBodies;

    fn maybe_tcx(&mut self) -> TyCtxt<'tcx> {
        self.cx.tcx
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        self.count(expr);
        if !Self::nests(expr) {
            return intravisit::walk_expr(self, expr);
        }
        self.depth += 1;
        self.shape.max_depth = self.shape.max_depth.max(self.depth);
        intravisit::walk_expr(self, expr);
        self.depth -= 1;
    }
}

fn line_count(cx: &LateContext<'_>, span: Span) -> usize {
    cx.sess()
        .source_map()
        .span_to_lines(span)
        .map_or(0, |file_lines| file_lines.lines.len())
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
