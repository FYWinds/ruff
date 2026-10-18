import MonacoEditor from "@monaco-editor/react";
import { AstralButton, Theme } from "shared";
import { ReadonlyFiles } from "../Playground";
import type { Example } from "../examples";
import { Suspense, use, useState } from "react";
import classNames from "classnames";

export enum SecondaryTool {
  AST = "AST",
  Tokens = "Tokens",
  Run = "Run",
  About = "About",
}

export type SecondaryPanelResult =
  null | { status: "ok"; content: string } | { status: "error"; error: string };

/** The example the current workspace was loaded from. */
export interface ActiveExample {
  example: Example;
  /** Whether the workspace differs from the example's files. */
  modified: boolean;
}

export interface SecondaryPanelProps {
  files: ReadonlyFiles;
  onRun(): Promise<string>;
  tool: SecondaryTool;
  result: SecondaryPanelResult;
  theme: Theme;
  activeExample: ActiveExample | null;
}

export default function SecondaryPanel({
  tool,
  result,
  files,
  onRun,
  theme,
  activeExample,
}: SecondaryPanelProps) {
  return (
    <div className="flex flex-col h-full">
      {tool === SecondaryTool.About ? (
        activeExample == null ? null : (
          <About activeExample={activeExample} theme={theme} />
        )
      ) : (
        <Content
          tool={tool}
          result={result}
          theme={theme}
          onRun={onRun}
          revision={files.revision}
        />
      )}
    </div>
  );
}

function About({
  activeExample: { example, modified },
  theme,
}: {
  activeExample: ActiveExample;
  theme: Theme;
}) {
  const sections = [
    `# ${example.title}${modified ? " (modified)" : ""}`,
    example.descriptionMarkdown,
  ];
  if (example.ts != null) {
    sections.push(`TypeScript equivalent: \`${example.ts}\``);
  }
  sections.push(
    `Deep link: \`?example=${example.slug}\` · ${example.phase === 0 ? "existing ty feature" : `PEP 827 phase ${example.phase}`}`,
  );

  return (
    <div className="flex grow">
      <MonacoEditor
        options={{
          readOnly: true,
          minimap: { enabled: false },
          fontSize: 14,
          roundedSelection: false,
          scrollBeyondLastLine: false,
          contextmenu: false,
          wordWrap: "on",
          lineNumbers: "off",
        }}
        language="markdown"
        value={sections.join("\n\n")}
        theme={theme === "light" ? "Ayu-Light" : "Ayu-Dark"}
      />
    </div>
  );
}

function Content({
  tool,
  result,
  theme,
  revision,
  onRun,
}: {
  tool: SecondaryTool;
  onRun(): Promise<string>;
  revision: number;
  result: SecondaryPanelResult;
  theme: Theme;
}) {
  if (result == null) {
    return "";
  } else {
    let language;
    switch (result.status) {
      case "ok":
        switch (tool) {
          case "AST":
            language = "RustPythonAst";
            break;

          case "Tokens":
            language = "RustPythonTokens";
            break;

          case "Run":
            return <Run theme={theme} onRun={onRun} key={`${revision}`} />;
        }

        return (
          <div className="flex grow">
            <MonacoEditor
              options={{
                readOnly: true,
                minimap: { enabled: false },
                fontSize: 14,
                roundedSelection: false,
                scrollBeyondLastLine: false,
                contextmenu: false,
              }}
              language={language}
              value={result.content}
              theme={theme === "light" ? "Ayu-Light" : "Ayu-Dark"}
            />
          </div>
        );
      case "error":
        return (
          <div className="flex grow">
            <code className="whitespace-pre-wrap text-gray-900 dark:text-gray-100">
              {result.error}
            </code>
          </div>
        );
    }
  }
}

function Run({ onRun, theme }: { onRun(): Promise<string>; theme: Theme }) {
  const [runOutput, setRunOutput] = useState<Promise<string> | null>(null);
  const handleRun = () => {
    setRunOutput(onRun());
  };

  if (runOutput == null) {
    return (
      <div className="flex flex-auto flex-col justify-center  items-center">
        <AstralButton
          type="button"
          className="flex-none leading-6 py-1.5 px-3 shadow-xs"
          onClick={handleRun}
        >
          <span
            className="inset-0 flex items-center justify-center"
            aria-hidden="false"
          >
            Run...
          </span>
        </AstralButton>
      </div>
    );
  }

  return (
    <Suspense
      fallback={<div className="text-center dark:text-white">Loading</div>}
    >
      <RunOutput theme={theme} runOutput={runOutput} />
    </Suspense>
  );
}

function RunOutput({
  runOutput,
  theme,
}: {
  theme: Theme;
  runOutput: Promise<string>;
}) {
  const output = use(runOutput);

  return (
    <pre
      className={classNames(
        "m-2",
        "text-sm",
        "whitespace-pre-wrap",
        "break-all",
        "overflow-auto",
        theme === "dark" ? "text-white" : null,
      )}
    >
      {output}
    </pre>
  );
}
