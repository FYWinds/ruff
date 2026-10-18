import examplesMarkdown from "../../../crates/ty_python_semantic/resources/mdtest/type_manipulation/examples.md?raw";
import { type Example, parseExamples } from "./parseExamples";
import { DEFAULT_SETTINGS } from "./settings";

export type { Example } from "./parseExamples";

/**
 * The playground examples, parsed from the mdtest suite that also checks their
 * `# revealed:` and `# error:` assertions.
 */
export const EXAMPLES: readonly Example[] = parseExamples(
  examplesMarkdown,
  DEFAULT_SETTINGS,
);

export function findExample(slug: string): Example | undefined {
  return EXAMPLES.find((example) => example.slug === slug);
}
