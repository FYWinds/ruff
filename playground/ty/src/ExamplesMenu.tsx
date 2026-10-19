import classNames from "classnames";
import {
  Header,
  Menu,
  MenuItem,
  MenuSection,
  MenuTrigger,
  Popover,
  Pressable,
  Text,
} from "react-aria-components";
import { AstralButton } from "shared";
import { type Example, phaseGroupLabel } from "./examples";

export interface ExamplesMenuProps {
  examples: readonly Example[];
  /** Slug of the example the current workspace was loaded from. */
  selected: string | null;
  onSelect(slug: string): void;
}

export default function ExamplesMenu({
  examples,
  selected,
  onSelect,
}: ExamplesMenuProps) {
  const phases = [
    ...Map.groupBy(examples, (example) => example.phase),
  ].toSorted(([a], [b]) => a - b);

  return (
    <MenuTrigger>
      <Pressable>
        <AstralButton
          type="button"
          className="relative flex-none leading-6 py-1.5 px-3 shadow-xs"
        >
          Examples
        </AstralButton>
      </Pressable>
      <Popover
        placement="bottom end"
        className="w-[28rem] max-w-[90vw] bg-white dark:bg-galaxy border border-gray-200 dark:border-comet rounded-md shadow-lg mt-1 z-10 overflow-auto"
      >
        <Menu
          aria-label="Examples"
          selectionMode="single"
          selectedKeys={selected == null ? [] : [selected]}
          onAction={(key) => onSelect(String(key))}
          className="font-sans p-1 outline-0 max-h-[inherit] overflow-auto"
        >
          {phases.map(([phase, phaseExamples]) => (
            <MenuSection key={phase} id={`phase-${phase}`}>
              <Header className="px-3 pt-2 pb-1 text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
                {phaseGroupLabel(phase)}
              </Header>
              {phaseExamples.map((example) => (
                <MenuItem
                  key={example.slug}
                  id={example.slug}
                  textValue={example.title}
                  className={({ isSelected }) =>
                    classNames(
                      "flex flex-col px-3 py-1.5 cursor-pointer outline-0 rounded",
                      "text-galaxy dark:text-white",
                      "hover:bg-gray-100 dark:hover:bg-space focus:bg-gray-100 dark:focus:bg-space",
                      isSelected && "font-semibold",
                    )
                  }
                >
                  <Text slot="label" className="text-sm">
                    {example.title}
                  </Text>
                  {example.ts != null ? (
                    <Text
                      slot="description"
                      className="text-xs font-mono text-gray-500 dark:text-gray-400 truncate"
                    >
                      TS: {example.ts}
                    </Text>
                  ) : null}
                </MenuItem>
              ))}
            </MenuSection>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}
