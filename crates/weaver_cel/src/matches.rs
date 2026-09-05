// SPDX-License-Identifier: Apache-2.0

//! The regex patterns an expression passes to `matches`.

use cel::common::ast::{Expr, IdedExpr, LiteralValue};

/// The name a CEL expression calls it by.
const MATCHES: &str = "matches";

/// The literal patterns an expression passes to `matches`.
///
/// A pattern built at run time is not visible here, so it stays a runtime
/// error.
pub(crate) fn literal_patterns(expression: &IdedExpr) -> Vec<&str> {
    let mut patterns = Vec::new();
    collect(expression, &mut patterns);
    patterns
}

fn collect<'a>(expression: &'a IdedExpr, patterns: &mut Vec<&'a str>) {
    match &expression.expr {
        Expr::Call(call) => {
            if call.func_name == MATCHES && call.target.is_some() {
                if let Some(Expr::Literal(LiteralValue::String(pattern))) =
                    call.args.first().map(|argument| &argument.expr)
                {
                    patterns.push(pattern.inner());
                }
            }
            if let Some(target) = &call.target {
                collect(target, patterns);
            }
            for argument in &call.args {
                collect(argument, patterns);
            }
        }
        Expr::Comprehension(comprehension) => {
            collect(&comprehension.iter_range, patterns);
            collect(&comprehension.accu_init, patterns);
            collect(&comprehension.loop_cond, patterns);
            collect(&comprehension.loop_step, patterns);
            collect(&comprehension.result, patterns);
        }
        Expr::List(list) => {
            for element in &list.elements {
                collect(element, patterns);
            }
        }
        Expr::Select(select) => collect(&select.operand, patterns),
        Expr::Ident(_) | Expr::Literal(_) | Expr::Map(_) | Expr::Struct(_) | Expr::Unspecified => {}
    }
}
