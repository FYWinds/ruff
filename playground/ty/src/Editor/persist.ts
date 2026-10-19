import { type Example, findExample } from "../examples";
import { decodePlayground, encodePlayground } from "./share";
import {
  generatePlaygroundMarkdown,
  generatePlaygroundMarkdownLink,
} from "./markdown";

interface Workspace {
  files: { [name: string]: string };

  // Name of the current file
  current: string;
}

export interface RestoredWorkspace {
  workspace: Workspace;

  // The example the workspace was loaded from, if any.
  example: Example | null;
}

/**
 * Generate a self-contained shareable URL for the workspace.
 */
async function shareUrl(workspace: Workspace): Promise<string> {
  const fragment = await encodePlayground(workspace);
  return `${window.location.origin}/#${fragment}`;
}

/**
 * Persist the workspace and copy a shareable URL to clipboard.
 */
export async function persist(workspace: Workspace): Promise<void> {
  const url = await shareUrl(workspace);
  await navigator.clipboard.writeText(url);
}

/**
 * Persist the workspace and copy a markdown link to clipboard.
 */
export async function copyAsMarkdownLink(workspace: Workspace): Promise<void> {
  const url = await shareUrl(workspace);
  await navigator.clipboard.writeText(generatePlaygroundMarkdownLink(url));
}

/**
 * Persist the workspace and copy markdown with code to clipboard.
 */
export async function copyAsMarkdown(workspace: Workspace): Promise<void> {
  const url = await shareUrl(workspace);
  await navigator.clipboard.writeText(
    generatePlaygroundMarkdown(workspace.files, url),
  );
}

/**
 * Restore the workspace, in order of precedence, from:
 *
 * 1. the share link in the URL fragment,
 * 2. the `?example=<slug>` query parameter,
 * 3. local storage.
 *
 * Returns `null` if there is nothing to restore.
 */
export async function restore(): Promise<RestoredWorkspace | null> {
  const fragment = window.location.hash.slice(1);
  if (fragment !== "") {
    try {
      const playground = await decodePlayground(fragment);
      if (playground != null) {
        return { workspace: playground, example: null };
      }
    } catch (error) {
      // oxlint-disable-next-line no-console
      console.warn("Failed to restore the shared workspace:", error);
    }
  }

  const slug = new URLSearchParams(window.location.search).get("example");
  if (slug != null) {
    const example = findExample(slug);
    if (example != null) {
      return { workspace: example.workspace, example };
    }

    // oxlint-disable-next-line no-console
    console.warn(
      `Unknown example '${slug}', restoring the workspace from local storage instead.`,
    );
  }

  // Otherwise, restore from local storage.
  return restoreLocal();
}

/**
 * Persists the workspace to local storage, together with the slug of the
 * example it was loaded from (if any).
 */
export function persistLocal(workspace: Workspace, example: string | null) {
  let totalLength = 0;
  for (const fileContent of Object.values(workspace.files)) {
    totalLength += fileContent.length;

    // Don't persist large files to local storage because they can exceed the local storage quota
    // The number here is picked rarely arbitrarily. Also note, JS uses UTF 16:
    // that means the limit here is strings larger than 1MB (because UTf 16 uses 2 bytes per character)
    if (totalLength > 500_000) {
      return;
    }
  }

  localStorage.setItem("workspace", JSON.stringify({ ...workspace, example }));
}

function restoreLocal(): RestoredWorkspace | null {
  const stored = localStorage.getItem("workspace");

  if (stored == null) {
    return null;
  }

  const { example, ...workspace }: Workspace & { example?: string | null } =
    JSON.parse(stored);

  return {
    workspace,
    example: example == null ? null : (findExample(example) ?? null),
  };
}
