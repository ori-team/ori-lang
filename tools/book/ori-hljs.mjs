/**
 * highlight.js language definition for Ori (.orl) / surface S3.
 * Keywords and comment style mirror docs/spec/02-lexical.md.
 */
export function registerOri(hljs) {
  const KEYWORDS = {
    keyword:
      "module import imports public struct enum apply use if elif else then " +
      "while for in match case end return const var try check async await " +
      "using and or not break continue",
    literal: "true false none some ok err",
    type:
      "int float bool string bytes void list map set optional result any",
    built_in: "main",
  };

  const COMMENT = hljs.COMMENT("--", "$");

  const F_STRING = {
    className: "string",
    begin: /f"/,
    end: /"/,
    illegal: "\\n",
    contains: [
      hljs.BACKSLASH_ESCAPE,
      {
        className: "subst",
        begin: /\{/,
        end: /\}/,
        contains: [
          {
            begin: /[a-zA-Z_][\w.]*/,
          },
        ],
      },
    ],
  };

  const NUMBER = {
    className: "number",
    relevance: 0,
    variants: [
      { begin: /\b0x[0-9a-fA-F]+\b/ },
      { begin: /\b\d+(\.\d+)?\b/ },
    ],
  };

  const PIPE = {
    className: "operator",
    begin: /\|>/,
    relevance: 10,
  };

  return {
    name: "Ori",
    aliases: ["orl", "ori"],
    keywords: KEYWORDS,
    contains: [
      COMMENT,
      F_STRING,
      hljs.QUOTE_STRING_MODE,
      NUMBER,
      PIPE,
      {
        className: "title.class",
        begin: /\b[A-Z][A-Za-z0-9_]*/,
        relevance: 0,
      },
      {
        className: "meta",
        begin: /@test\b/,
      },
    ],
  };
}
