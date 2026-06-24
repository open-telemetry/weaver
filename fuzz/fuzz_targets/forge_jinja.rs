#![no_main]
use libfuzzer_sys::fuzz_target;
use minijinja::syntax::SyntaxConfig;
use minijinja::{context, Environment};
use std::borrow::Cow;
use weaver_forge::{
    BLOCK_END, BLOCK_START, COMMEND_END, COMMENT_START, VARIABLE_END, VARIABLE_START,
};

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let mut env = Environment::new();

    // Mirror weaver's minijinja setup (see TemplateEngine::template_engine).
    let syntax = match SyntaxConfig::builder()
        .block_delimiters(Cow::Borrowed(BLOCK_START), Cow::Borrowed(BLOCK_END))
        .variable_delimiters(Cow::Borrowed(VARIABLE_START), Cow::Borrowed(VARIABLE_END))
        .comment_delimiters(Cow::Borrowed(COMMENT_START), Cow::Borrowed(COMMEND_END))
        .build()
    {
        Ok(s) => s,
        Err(_) => return,
    };
    env.set_syntax(syntax);

    minijinja_contrib::add_to_environment(&mut env);
    env.set_unknown_method_callback(minijinja_contrib::pycompat::unknown_method_callback);

    // Parse phase.
    if env.add_template_owned("t", source.to_owned()).is_err() {
        return;
    }

    // Render phase with an empty context — exercises control flow, loops, filters.
    if let Ok(tmpl) = env.get_template("t") {
        let _ = tmpl.render(context! {});
    }
});
