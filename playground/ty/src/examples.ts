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

/**
 * Examples with this `phase` are showcases that combine the operators of all
 * PEP 827 phases. They come after phase 5 in the menu, so the menu still reads
 * from the basics to the larger programs that build on them.
 */
export const SHOWCASE_PHASE = 6;

/** Heading of a phase's group in the examples menu. */
export function phaseGroupLabel(phase: number): string {
  if (phase === 0) {
    return "Existing ty features";
  }
  return phase === SHOWCASE_PHASE ? "Showcase" : `PEP 827 · phase ${phase}`;
}

/** Short description of a phase for the About panel. */
export function phaseDescription(phase: number): string {
  if (phase === 0) {
    return "existing ty feature";
  }
  return phase === SHOWCASE_PHASE
    ? "PEP 827 showcase (combines all phases)"
    : `PEP 827 phase ${phase}`;
}
