# Ori – Shiki Syntax Highlighting

This directory contains the [TextMate grammar](ori.tmLanguage.json) for the **Ori** programming language, packaged for use with [Shiki](https://shiki.matsu.io/) — the syntax highlighter used by [Astro Starlight](https://starlight.astro.build/).

> **Note:** This grammar is an exact copy of the one used by the [VS Code extension](../vscode-orl/syntaxes/ori.tmLanguage.json). If you update the grammar in the VS Code extension, make sure to copy the changes here as well.

## Usage in Astro Starlight

### 1. Copy the grammar file into your website project

Place `ori.tmLanguage.json` somewhere accessible in your Astro project, for example:

```
src/grammars/ori.tmLanguage.json
```

### 2. Register the language in `astro.config.mjs`

Import the grammar JSON and pass it to Starlight's Shiki configuration via the `langs` option:

```js
// astro.config.mjs
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import oriGrammar from "./src/grammars/ori.tmLanguage.json";

export default defineConfig({
  integrations: [
    starlight({
      title: "Ori Language",
      expressiveCode: {
        shiki: {
          langs: [oriGrammar],
        },
      },
      // ... other Starlight options
    }),
  ],
});
```

> **Tip:** If your project does not use `resolveJsonModule` in `tsconfig.json`, you can read the grammar with `fs.readFileSync` and `JSON.parse` instead.

### 3. Use `ori` code fences in Markdown / MDX

Once registered, fenced code blocks with the `ori` language identifier will be syntax-highlighted automatically:

````md
```ori
-- Hello World in Ori
func main()
  const message = "Hello, World!"
  print(message)
end
```
````

This renders with proper highlighting for keywords, strings, comments, types, numbers, and function names.

## What the grammar highlights

| Scope                        | Examples                                       |
| ---------------------------- | ---------------------------------------------- |
| `keyword.control`            | `func`, `if`, `else`, `return`, `match`, …     |
| `storage.type`               | `const`, `var`, `int`, `string`, `list`, …     |
| `entity.name.type`           | `PascalCase` identifiers (e.g. `MyStruct`)     |
| `entity.name.function`       | Identifier after `func` keyword                |
| `string.quoted.double`       | `"double-quoted strings"`                      |
| `constant.character.escape`  | Escape sequences inside strings (`\n`, `\"`)   |
| `constant.numeric`           | `42`, `3.14`                                   |
| `comment.line.double-dash`   | `-- single-line comment`                       |
| `comment.block`              | `--| block comment |--`                        |
