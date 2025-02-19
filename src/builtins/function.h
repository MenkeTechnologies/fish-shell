// Prototypes for executing builtin_function function.
#ifndef FISH_BUILTIN_FUNCTION_H
#define FISH_BUILTIN_FUNCTION_H

#include "../ast.h"
#include "../common.h"
#include "../parse_tree.h"

class parser_t;
struct io_streams_t;

int builtin_function(parser_t &parser, io_streams_t &streams, const std::vector<wcstring> &c_args,
                     const parsed_source_ref_t &source, const ast::block_statement_t &func_node);
#endif
