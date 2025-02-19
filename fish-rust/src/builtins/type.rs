use libc::c_int;
use libc::isatty;
use libc::STDOUT_FILENO;

use crate::builtins::shared::{
    builtin_missing_argument, builtin_print_help, builtin_unknown_option, io_streams_t,
    BUILTIN_ERR_COMBO, STATUS_CMD_ERROR, STATUS_CMD_OK, STATUS_INVALID_ARGS,
};
use crate::ffi::parser_t;
use crate::ffi::Repin;
use crate::ffi::{
    builtin_exists, colorize_shell, function_get_annotated_definition,
    function_get_copy_definition_file, function_get_copy_definition_lineno,
    function_get_definition_file, function_get_definition_lineno, function_get_props_autoload,
    function_is_copy,
};
use crate::path::{path_get_path, path_get_paths};
use crate::wchar::{wstr, WString, L};
use crate::wchar_ffi::WCharFromFFI;
use crate::wchar_ffi::WCharToFFI;
use crate::wgetopt::{wgetopter_t, wopt, woption, woption_argument_t};
use crate::wutil::{sprintf, wgettext, wgettext_fmt};

#[derive(Default)]
struct type_cmd_opts_t {
    all: bool,
    short_output: bool,
    no_functions: bool,
    get_type: bool,
    path: bool,
    force_path: bool,
    print_help: bool,
    query: bool,
}

pub fn r#type(
    parser: &mut parser_t,
    streams: &mut io_streams_t,
    argv: &mut [&wstr],
) -> Option<c_int> {
    let cmd = argv[0];
    let argc = argv.len();
    let print_hints = false;
    let mut opts: type_cmd_opts_t = Default::default();

    const shortopts: &wstr = L!(":hasftpPq");
    const longopts: &[woption] = &[
        wopt(L!("help"), woption_argument_t::no_argument, 'h'),
        wopt(L!("all"), woption_argument_t::no_argument, 'a'),
        wopt(L!("short"), woption_argument_t::no_argument, 's'),
        wopt(L!("no-functions"), woption_argument_t::no_argument, 'f'),
        wopt(L!("type"), woption_argument_t::no_argument, 't'),
        wopt(L!("path"), woption_argument_t::no_argument, 'p'),
        wopt(L!("force-path"), woption_argument_t::no_argument, 'P'),
        wopt(L!("query"), woption_argument_t::no_argument, 'q'),
        wopt(L!("quiet"), woption_argument_t::no_argument, 'q'),
    ];

    let mut w = wgetopter_t::new(shortopts, longopts, argv);
    while let Some(c) = w.wgetopt_long() {
        match c {
            'a' => opts.all = true,
            's' => opts.short_output = true,
            'f' => opts.no_functions = true,
            't' => opts.get_type = true,
            'p' => opts.path = true,
            'P' => opts.force_path = true,
            'q' => opts.query = true,
            'h' => {
                builtin_print_help(parser, streams, cmd);
                return STATUS_CMD_OK;
            }
            ':' => {
                builtin_missing_argument(parser, streams, cmd, argv[w.woptind - 1], print_hints);
                return STATUS_INVALID_ARGS;
            }
            '?' => {
                builtin_unknown_option(parser, streams, cmd, argv[w.woptind - 1], print_hints);
                return STATUS_INVALID_ARGS;
            }
            _ => {
                panic!("unexpected retval from wgeopter.next()");
            }
        }
    }

    if opts.query as i64 + opts.path as i64 + opts.get_type as i64 + opts.force_path as i64 > 1 {
        streams.err.append(wgettext_fmt!(BUILTIN_ERR_COMBO, cmd));
        return STATUS_INVALID_ARGS;
    }

    let mut res = false;

    let optind = w.woptind;
    for arg in argv.iter().take(argc).skip(optind) {
        let mut found = 0;
        if !opts.force_path && !opts.no_functions {
            let props = function_get_props_autoload(&arg.to_ffi(), parser.pin());
            if !props.is_null() {
                found += 1;
                res = true;
                // Early out - query means *any of the args exists*.
                if opts.query {
                    return STATUS_CMD_OK;
                }
                if !opts.get_type {
                    let path = function_get_definition_file(&props).from_ffi();
                    let mut comment = WString::new();

                    if path.is_empty() {
                        comment.push_utfstr(&wgettext_fmt!("Defined interactively"));
                    } else if path == "-" {
                        comment.push_utfstr(&wgettext_fmt!("Defined via `source`"));
                    } else {
                        let lineno: i32 = i32::from(function_get_definition_lineno(&props));
                        comment.push_utfstr(&wgettext_fmt!(
                            "Defined in %ls @ line %d",
                            path,
                            lineno
                        ));
                    }

                    if function_is_copy(&props) {
                        let path = function_get_copy_definition_file(&props).from_ffi();
                        if path.is_empty() {
                            comment.push_utfstr(&wgettext_fmt!(", copied interactively"));
                        } else if path == "-" {
                            comment.push_utfstr(&wgettext_fmt!(", copied via `source`"));
                        } else {
                            let lineno: i32 =
                                i32::from(function_get_copy_definition_lineno(&props));
                            comment.push_utfstr(&wgettext_fmt!(
                                ", copied in %ls @ line %d",
                                path,
                                lineno
                            ));
                        }
                    }
                    if opts.path {
                        if function_is_copy(&props) {
                            let path = function_get_copy_definition_file(&props).from_ffi();
                            streams.out.append(path);
                        } else {
                            streams.out.append(path);
                        }
                        streams.out.append(L!("\n"));
                    } else if !opts.short_output {
                        streams.out.append(wgettext_fmt!("%ls is a function", arg));
                        streams.out.append(wgettext_fmt!(" with definition"));
                        streams.out.append(L!("\n"));
                        let mut def = WString::new();
                        def.push_utfstr(&sprintf!(
                            "# %ls\n%ls",
                            comment,
                            function_get_annotated_definition(&props, &arg.to_ffi()).from_ffi()
                        ));

                        if !streams.out_is_redirected && unsafe { isatty(STDOUT_FILENO) == 1 } {
                            let col = colorize_shell(&def.to_ffi(), parser.pin()).from_ffi();
                            streams.out.append(col);
                        } else {
                            streams.out.append(def);
                        }
                    } else {
                        streams.out.append(wgettext_fmt!("%ls is a function", arg));
                        streams.out.append(wgettext_fmt!(" (%ls)\n", comment));
                    }
                } else if opts.get_type {
                    streams.out.append(L!("function\n"));
                }
                if !opts.all {
                    continue;
                }
            }
        }

        if !opts.force_path && builtin_exists(&arg.to_ffi()) {
            found += 1;
            res = true;
            if opts.query {
                return STATUS_CMD_OK;
            }
            if !opts.get_type {
                streams.out.append(wgettext_fmt!("%ls is a builtin\n", arg));
            } else if opts.get_type {
                streams.out.append(wgettext!("builtin\n"));
            }
            if !opts.all {
                continue;
            }
        }

        let paths = if opts.all {
            path_get_paths(arg, &*parser.get_vars())
        } else {
            match path_get_path(arg, &*parser.get_vars()) {
                Some(p) => vec![p],
                None => vec![],
            }
        };

        for path in paths.iter() {
            found += 1;
            res = true;
            if opts.query {
                return STATUS_CMD_OK;
            }
            if !opts.get_type {
                if opts.path || opts.force_path {
                    streams.out.append(sprintf!("%ls\n", path));
                } else {
                    streams.out.append(wgettext_fmt!("%ls is %ls\n", arg, path));
                }
            } else if opts.get_type {
                streams.out.append(L!("file\n"));
                break;
            }
            if !opts.all {
                // We need to *break* out of this loop
                // and continue on to the next argument,
                // otherwise we would print every other path
                // for a given argument.
                break;
            }
        }

        if found == 0 && !opts.query && !opts.path {
            streams.err.append(wgettext_fmt!(
                "%ls: Could not find '%ls'\n",
                L!("type"),
                arg
            ));
        }
    }

    if res {
        STATUS_CMD_OK
    } else {
        STATUS_CMD_ERROR
    }
}
