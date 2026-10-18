import { Icons, SideBar, SideBarEntry } from "shared";
import { SecondaryTool } from "./SecondaryPanel";

interface Props {
  selected: SecondaryTool | null;

  /** Title of the About entry; the entry is hidden when `null`. */
  aboutTitle: string | null;

  onSelected(tool: SecondaryTool): void;
}

export default function SecondarySideBar({
  selected,
  aboutTitle,
  onSelected,
}: Props) {
  return (
    <SideBar position="right">
      {aboutTitle != null ? (
        <SideBarEntry
          title={aboutTitle}
          position={"right"}
          selected={selected === SecondaryTool.About}
          onClick={() => onSelected(SecondaryTool.About)}
        >
          <Icons.Info />
        </SideBarEntry>
      ) : null}
      <SideBarEntry
        title="AST"
        position={"right"}
        selected={selected === SecondaryTool.AST}
        onClick={() => onSelected(SecondaryTool.AST)}
      >
        <Icons.Structure />
      </SideBarEntry>

      <SideBarEntry
        title="Tokens"
        position={"right"}
        selected={selected === SecondaryTool.Tokens}
        onClick={() => onSelected(SecondaryTool.Tokens)}
      >
        <Icons.Token />
      </SideBarEntry>
      <SideBarEntry
        title="Run"
        position={"right"}
        selected={selected === SecondaryTool.Run}
        onClick={() => onSelected(SecondaryTool.Run)}
      >
        <Icons.Run />
      </SideBarEntry>
    </SideBar>
  );
}
