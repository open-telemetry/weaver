// SPDX-License-Identifier: Apache-2.0

//! Collects the variables an expression reads from its environment.

use std::collections::BTreeSet;

use cel::common::ast::{EntryExpr, Expr, IdedExpr};

/// The variables an expression reads that nothing in it binds.
///
/// `Program::references().variables()` includes a comprehension's loop
/// variable, so it cannot be used for this.
pub(crate) fn free_variables(expression: &IdedExpr) -> BTreeSet<String> {
    let mut free = BTreeSet::new();
    let mut bound = Vec::new();
    collect(expression, &mut bound, &mut free);
    free
}

fn collect(expression: &IdedExpr, bound: &mut Vec<String>, free: &mut BTreeSet<String>) {
    match &expression.expr {
        // A leading `@` marks a parser-generated name, not a caller's variable.
        Expr::Ident(name) => {
            if !name.starts_with('@') && !bound.iter().any(|held| held == name) {
                let _ = free.insert(name.clone());
            }
        }
        Expr::Call(call) => {
            if let Some(target) = &call.target {
                collect(target, bound, free);
            }
            for argument in &call.args {
                collect(argument, bound, free);
            }
        }
        Expr::Comprehension(comprehension) => {
            collect(&comprehension.iter_range, bound, free);
            collect(&comprehension.accu_init, bound, free);
            let depth = bound.len();
            bound.push(comprehension.iter_var.clone());
            if let Some(second) = &comprehension.iter_var2 {
                bound.push(second.clone());
            }
            bound.push(comprehension.accu_var.clone());
            collect(&comprehension.loop_cond, bound, free);
            collect(&comprehension.loop_step, bound, free);
            collect(&comprehension.result, bound, free);
            bound.truncate(depth);
        }
        Expr::List(list) => {
            for element in &list.elements {
                collect(element, bound, free);
            }
        }
        Expr::Map(map) => {
            for entry in &map.entries {
                collect_entry(&entry.expr, bound, free);
            }
        }
        Expr::Struct(structure) => {
            for entry in &structure.entries {
                collect_entry(&entry.expr, bound, free);
            }
        }
        Expr::Select(select) => collect(&select.operand, bound, free),
        Expr::Literal(_) | Expr::Unspecified => {}
    }
}

fn collect_entry(entry: &EntryExpr, bound: &mut Vec<String>, free: &mut BTreeSet<String>) {
    match entry {
        EntryExpr::StructField(field) => collect(&field.value, bound, free),
        EntryExpr::MapEntry(map_entry) => {
            collect(&map_entry.key, bound, free);
            collect(&map_entry.value, bound, free);
        }
    }
}
