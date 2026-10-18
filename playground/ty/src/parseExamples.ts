/**
 * Parser for the playground examples file
 * (`crates/ty_python_semantic/resources/mdtest/type_manipulation/examples.md`).
 *
 * The file is a regular mdtest suite. The playground supports a strict subset of
 * the mdtest syntax and throws on anything outside of it, rather than silently
 * producing a wrong example:
 *
 * - Every `##` heading is one example. The `#` heading is the file title. Deeper
 *   headings (nested tests) are not supported.
 * - `[//]: # (playground: slug=..., phase=..., ts="...")` on its own line right
 *   after the heading holds the metadata. `slug` and `phase` are required, `ts`
 *   is optional. Values must not contain `)`.
 * - The prose between the heading and the first code block is the description.
 * - Unlabeled `py` blocks are merged into `main.py`. A line `` `name.py`: ``
 *   before a block names that file.
 * - An optional `toml` block is converted to `ty.json`, layered on top of the
 *   playground's default settings.
 * - `ty_extensions` doesn't exist at runtime, so it may only be imported inside
 *   `if TYPE_CHECKING:` (otherwise the Run button would fail).
 *
 * This module is pure (no Vite-specific imports) so that `vite.config.ts` can
 * validate the examples when the file is loaded, failing the build or showing
 * the dev server's error overlay.
 */
import { parse as parseToml } from "smol-toml";
import { SETTINGS_FILE_NAME } from "./settings";

export interface ExampleWorkspace {
  files: { [name: string]: string };
  current: string;
}

export interface Example {
  slug: string;
  title: string;
  phase: number;
  ts?: string;
  descriptionMarkdown: string;
  workspace: ExampleWorkspace;
}

const MAIN_FILE = "main.py";

const METADATA_KEYS: Record<string, true> = {
  slug: true,
  phase: true,
  ts: true,
};
const SLUG_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
// Top-level tables of the mdtest configuration that map onto `ty.json`. Others
// (`project`, `dependency-metadata`, `system`, `log`, `verbose`) only make
// sense for the test runner.
const TOML_TABLES: Record<string, true> = {
  environment: true,
  rules: true,
  analysis: true,
};
// `[environment]` keys pointing at mdtest's in-memory file system.
const UNSUPPORTED_ENVIRONMENT_KEYS: Record<string, true> = {
  python: true,
  typeshed: true,
  "extra-paths": true,
};
// HTML comments that are formatting directives and don't affect the example.
const IGNORED_COMMENT = /^<!--\s*blacken-docs:(?:on|off)\s*-->$/;

class ExamplesSyntaxError extends Error {
  constructor(line: number, message: string) {
    super(`examples.md:${line}: ${message}`);
    this.name = "ExamplesSyntaxError";
  }
}

interface PendingExample {
  title: string;
  line: number;
  metadata: Map<string, string> | null;
  description: string[];
  sawCode: boolean;
  mainBlocks: string[];
  files: Map<string, string>;
  toml: string | null;
}

/**
 * Parses the examples file.
 *
 * @param markdown The content of `examples.md`.
 * @param defaultSettings The playground's default `ty.json` content. The
 *   `toml` block of an example is merged on top of it.
 */
export function parseExamples(
  markdown: string,
  defaultSettings: string,
): Example[] {
  const lines = markdown.replace(/\r\n?/g, "\n").split("\n");
  const examples: Example[] = [];
  const slugs = new Set<string>();

  let current: PendingExample | null = null;
  // `[line, name]` of a `` `name.py`: `` marker waiting for its code block.
  let pendingPath: [number, string] | null = null;

  const finish = () => {
    if (current == null) {
      return;
    }
    if (pendingPath != null) {
      throw new ExamplesSyntaxError(
        pendingPath[0],
        `file name \`${pendingPath[1]}\` is not followed by a code block`,
      );
    }
    const example = buildExample(current, defaultSettings);
    if (slugs.has(example.slug)) {
      throw new ExamplesSyntaxError(
        current.line,
        `duplicate slug \`${example.slug}\``,
      );
    }
    slugs.add(example.slug);
    examples.push(example);
    current = null;
  };

  let index = 0;
  while (index < lines.length) {
    const lineNumber = index + 1;
    const line = lines[index];

    const fence = /^(`{3,}|~{3,})(.*)$/.exec(line);
    if (fence != null) {
      const [, marker, info] = fence;
      const end = findFenceEnd(lines, index + 1, marker);
      if (end === -1) {
        throw new ExamplesSyntaxError(lineNumber, "unterminated code block");
      }
      const content = lines.slice(index + 1, end).join("\n") + "\n";

      if (current == null) {
        throw new ExamplesSyntaxError(
          lineNumber,
          "code blocks must be inside a `##` example section",
        );
      }

      const path = pendingPath;
      pendingPath = null;
      addCodeBlock(
        current,
        lineNumber,
        info.trim(),
        content,
        path?.[1] ?? null,
      );
      index = end + 1;
      continue;
    }

    const heading = /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (heading != null) {
      const level = heading[1].length;
      if (level === 1) {
        if (current != null || examples.length > 0) {
          throw new ExamplesSyntaxError(
            lineNumber,
            "only a single `#` title heading at the top of the file is supported",
          );
        }
      } else if (level === 2) {
        finish();
        current = {
          title: heading[2],
          line: lineNumber,
          metadata: null,
          description: [],
          sawCode: false,
          mainBlocks: [],
          files: new Map(),
          toml: null,
        };
      } else {
        throw new ExamplesSyntaxError(
          lineNumber,
          `nested headings (\`${heading[1]}\`) are not supported; every \`##\` section is one example`,
        );
      }
      index++;
      continue;
    }

    const trimmed = line.trim();

    if (trimmed.startsWith("<!--")) {
      const commentEnd = findCommentEnd(lines, index);
      if (commentEnd === -1) {
        throw new ExamplesSyntaxError(lineNumber, "unterminated HTML comment");
      }
      const comment = lines
        .slice(index, commentEnd + 1)
        .join("\n")
        .trim();
      if (!comment.endsWith("-->")) {
        throw new ExamplesSyntaxError(
          lineNumber,
          "HTML comments must be on their own lines",
        );
      }

      if (!IGNORED_COMMENT.test(comment)) {
        throw new ExamplesSyntaxError(
          lineNumber,
          `unsupported HTML comment \`${comment.split("\n")[0]}\``,
        );
      }
      index = commentEnd + 1;
      continue;
    }

    // Markdown link-reference comment: `[//]: # (...)`.
    const linkComment = /^\[\/\/\]:\s*#\s*\((.*)\)$/.exec(trimmed);
    if (linkComment != null) {
      const playground = /^playground:(.*)$/.exec(linkComment[1].trim());
      if (playground != null) {
        if (current == null) {
          throw new ExamplesSyntaxError(
            lineNumber,
            "playground metadata must follow a `##` heading",
          );
        }
        if (current.metadata != null) {
          throw new ExamplesSyntaxError(
            lineNumber,
            "duplicate playground metadata comment",
          );
        }
        if (current.description.some((l) => l.trim() !== "")) {
          throw new ExamplesSyntaxError(
            lineNumber,
            "playground metadata must come directly after the `##` heading",
          );
        }
        current.metadata = parseMetadata(lineNumber, playground[1]);
      }
      // Other link-reference comments are invisible in rendered Markdown and
      // are not part of the description either.
      index++;
      continue;
    }

    const pathMarker = /^`([^`]+)`:$/.exec(trimmed);
    if (pathMarker != null) {
      if (current == null) {
        throw new ExamplesSyntaxError(
          lineNumber,
          "file names must be inside a `##` example section",
        );
      }
      if (pendingPath != null) {
        throw new ExamplesSyntaxError(
          pendingPath[0],
          `file name \`${pendingPath[1]}\` is not followed by a code block`,
        );
      }
      pendingPath = [lineNumber, pathMarker[1]];
      index++;
      continue;
    }

    if (trimmed !== "" && pendingPath != null) {
      throw new ExamplesSyntaxError(
        pendingPath[0],
        `file name \`${pendingPath[1]}\` must be directly followed by its code block`,
      );
    }

    // Prose before the first `##` belongs to the file title; prose after the
    // first code block of an example is documentation for the mdtest only.
    if (current != null && !current.sawCode) {
      current.description.push(line);
    }
    index++;
  }

  finish();

  if (examples.length === 0) {
    throw new Error("examples.md: no `##` examples found");
  }

  return examples;
}

function findFenceEnd(lines: string[], start: number, marker: string): number {
  const closing = new RegExp(
    `^${marker[0] === "`" ? "`" : "~"}{${marker.length},}\\s*$`,
  );
  for (let i = start; i < lines.length; i++) {
    if (closing.test(lines[i])) {
      return i;
    }
  }
  return -1;
}

function findCommentEnd(lines: string[], start: number): number {
  for (let i = start; i < lines.length; i++) {
    if (lines[i].includes("-->")) {
      return i;
    }
  }
  return -1;
}

function parseMetadata(line: number, body: string): Map<string, string> {
  const metadata = new Map<string, string>();
  const pair =
    /\s*([a-z][a-z0-9-]*)\s*=\s*(?:"((?:[^"\\]|\\.)*)"|([^,"\s][^,]*?))\s*(?:,|$)/y;
  const source = body.trim();
  let offset = 0;

  while (offset < source.length) {
    pair.lastIndex = offset;
    const match = pair.exec(source);
    if (match == null) {
      throw new ExamplesSyntaxError(
        line,
        `invalid playground metadata near \`${source.slice(offset)}\`; expected \`key=value\` or \`key="value"\` pairs separated by commas`,
      );
    }
    const [, key, quoted, bare] = match;
    if (!Object.hasOwn(METADATA_KEYS, key)) {
      throw new ExamplesSyntaxError(
        line,
        `unknown playground metadata key \`${key}\` (supported: ${Object.keys(METADATA_KEYS).join(", ")})`,
      );
    }
    if (metadata.has(key)) {
      throw new ExamplesSyntaxError(
        line,
        `duplicate playground metadata key \`${key}\``,
      );
    }
    metadata.set(
      key,
      quoted != null ? quoted.replace(/\\(.)/g, "$1") : bare.trim(),
    );
    offset = pair.lastIndex;
  }

  return metadata;
}

function addCodeBlock(
  example: PendingExample,
  line: number,
  info: string,
  content: string,
  path: string | null,
) {
  example.sawCode = true;
  const [language, ...options] = info.split(/\s+/).filter((t) => t !== "");

  if (options.length > 0) {
    throw new ExamplesSyntaxError(
      line,
      `code block options (\`${options.join(" ")}\`) are not supported`,
    );
  }

  switch (language) {
    case "py":
    case "pyi": {
      if (path == null) {
        if (language === "pyi") {
          throw new ExamplesSyntaxError(
            line,
            "`pyi` blocks need a file name (`` `name.pyi`: ``)",
          );
        }
        example.mainBlocks.push(content);
        return;
      }
      validatePath(line, path, language);
      if (path === MAIN_FILE) {
        throw new ExamplesSyntaxError(
          line,
          `\`${MAIN_FILE}\` is reserved for the unlabeled \`py\` blocks`,
        );
      }
      if (example.files.has(path)) {
        throw new ExamplesSyntaxError(line, `duplicate file \`${path}\``);
      }
      example.files.set(path, content);
      return;
    }
    case "toml": {
      if (path != null) {
        throw new ExamplesSyntaxError(
          line,
          `configuration files (\`${path}\`) are not supported; use an unlabeled \`toml\` block`,
        );
      }
      if (example.toml != null) {
        throw new ExamplesSyntaxError(
          line,
          "only one `toml` configuration block per example is supported",
        );
      }
      example.toml = content;
      return;
    }
    default:
      throw new ExamplesSyntaxError(
        line,
        `unsupported code block \`${info === "" ? "(no language)" : language}\`; expected \`py\`, \`pyi\` or \`toml\``,
      );
  }
}

function validatePath(line: number, path: string, language: string) {
  if (
    path.startsWith("/") ||
    path.split("/").some((part) => part === "" || part === "." || part === "..")
  ) {
    throw new ExamplesSyntaxError(
      line,
      `file name \`${path}\` must be a relative path inside the project`,
    );
  }
  if (!path.endsWith(`.${language}`)) {
    throw new ExamplesSyntaxError(
      line,
      `file name \`${path}\` doesn't match the \`${language}\` code block`,
    );
  }
}

function buildExample(
  pending: PendingExample,
  defaultSettings: string,
): Example {
  const { metadata, line, title } = pending;
  if (metadata == null) {
    throw new ExamplesSyntaxError(
      line,
      `example \`${title}\` is missing its \`[//]: # (playground: slug=..., phase=...)\` line`,
    );
  }

  const slug = metadata.get("slug");
  if (slug == null || !SLUG_PATTERN.test(slug)) {
    throw new ExamplesSyntaxError(
      line,
      `example \`${title}\` needs a \`slug\` of lowercase words separated by \`-\``,
    );
  }

  const phaseText = metadata.get("phase");
  if (phaseText == null || !/^\d+$/.test(phaseText)) {
    throw new ExamplesSyntaxError(
      line,
      `example \`${slug}\` needs a numeric \`phase\``,
    );
  }

  if (pending.mainBlocks.length === 0) {
    throw new ExamplesSyntaxError(
      line,
      `example \`${slug}\` has no unlabeled \`py\` block for \`${MAIN_FILE}\``,
    );
  }

  const files: { [name: string]: string } = {
    [MAIN_FILE]: pending.mainBlocks.join("\n"),
  };
  for (const [name, content] of pending.files) {
    files[name] = content;
  }

  for (const [name, content] of Object.entries(files)) {
    checkTyExtensionsImports(slug, name, content);
  }

  files[SETTINGS_FILE_NAME] =
    pending.toml == null
      ? defaultSettings
      : tomlToSettings(line, slug, pending.toml, defaultSettings);

  const ts = metadata.get("ts");

  return {
    slug,
    title,
    phase: Number(phaseText),
    ...(ts == null || ts === "" ? {} : { ts }),
    descriptionMarkdown: pending.description.join("\n").trim(),
    workspace: { files, current: MAIN_FILE },
  };
}

function tomlToSettings(
  line: number,
  slug: string,
  toml: string,
  defaultSettings: string,
): string {
  let config: Record<string, unknown>;
  try {
    config = parseToml(toml);
  } catch (error) {
    throw new ExamplesSyntaxError(
      line,
      `invalid \`toml\` block in example \`${slug}\`: ${error instanceof Error ? error.message : error}`,
    );
  }

  // Every supported key is a table whose entries are scalars or arrays, so a
  // two-level merge onto the defaults is exact.
  const settings: Record<string, Record<string, unknown>> = JSON.parse(
    defaultSettings,
  );
  for (const [key, table] of Object.entries(config)) {
    if (!Object.hasOwn(TOML_TABLES, key)) {
      throw new ExamplesSyntaxError(
        line,
        `\`${key}\` in example \`${slug}\` is mdtest-only and not supported by the playground`,
      );
    }
    if (typeof table !== "object" || table == null || Array.isArray(table)) {
      throw new ExamplesSyntaxError(
        line,
        `\`${key}\` in example \`${slug}\` must be a table`,
      );
    }
    for (const entry of Object.keys(table)) {
      if (
        key === "environment" &&
        Object.hasOwn(UNSUPPORTED_ENVIRONMENT_KEYS, entry)
      ) {
        throw new ExamplesSyntaxError(
          line,
          `\`environment.${entry}\` in example \`${slug}\` is mdtest-only and not supported by the playground`,
        );
      }
    }
    settings[key] = { ...settings[key], ...table };
  }

  return JSON.stringify(settings, null, 4);
}

/**
 * `ty_extensions` only exists for the type checker. Every import of it must be
 * nested in an `if TYPE_CHECKING:` block, or running the example fails.
 */
function checkTyExtensionsImports(slug: string, file: string, source: string) {
  const lines = source.split("\n");

  lines.forEach((line, index) => {
    if (!/^\s*(?:from\s+ty_extensions\b|import\s+ty_extensions\b)/.test(line)) {
      return;
    }

    const indent = line.length - line.trimStart().length;
    let guarded = false;
    for (let i = index - 1; i >= 0; i--) {
      const parent = lines[i];
      if (parent.trim() === "" || parent.trim().startsWith("#")) {
        continue;
      }
      if (parent.length - parent.trimStart().length < indent) {
        guarded = /^\s*if\s+(?:typing\.)?TYPE_CHECKING\s*:\s*(?:#.*)?$/.test(
          parent,
        );
        break;
      }
    }

    if (!guarded) {
      throw new Error(
        `examples.md: example \`${slug}\`, ${file}:${index + 1}: \`ty_extensions\` must only be imported inside \`if TYPE_CHECKING:\` (it doesn't exist at runtime)`,
      );
    }
  });
}
