# Template Engine

Status: **Deprecated (see weaver_template_engine)**

Important Note: Tera doesn't have a good error handling mechanism. Tera2 is
under development to address this issue but doesn't have an ETA yet. So the
plan is to use the crate MiniJinja for the template engine as it has a better
error handling mechanism.

This crate extends the `tera` template engine with custom filters and
functions. 

This template engine is not yet production ready as the error handling
is not satisfactory. A new version of this engine `tera2` is under
development and should fix the error handling issues.