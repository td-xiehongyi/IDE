import { useState } from "react";

export function useWorkbenchLayout() {
  const [assistantOpen, setAssistantOpen] = useState(false);
  const [explorerCollapsed, setExplorerCollapsed] = useState(false);
  const [assistantCollapsed, setAssistantCollapsed] = useState(false);
  const [activityCollapsed, setActivityCollapsed] = useState(false);
  const [leftWidth, setLeftWidth] = useState(260);
  const [rightWidth, setRightWidth] = useState(360);

  return {
    assistantOpen,
    setAssistantOpen,
    explorerCollapsed,
    setExplorerCollapsed,
    assistantCollapsed,
    setAssistantCollapsed,
    activityCollapsed,
    setActivityCollapsed,
    leftWidth,
    setLeftWidth,
    rightWidth,
    setRightWidth,
  };
}
