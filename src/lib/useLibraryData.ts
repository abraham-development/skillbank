// Datos compartidos por la barra lateral y la biblioteca.
import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { api } from "./api";

export function useLibraryData() {
  const skills = useQuery({ queryKey: ["bank", "list"], queryFn: api.bankList });
  const tags = useQuery({ queryKey: ["bank", "tags"], queryFn: api.bankTags });
  const installs = useQuery({ queryKey: ["installs"], queryFn: () => api.installsList() });

  const { installsBySkill, outdatedBySkill } = useMemo(() => {
    const all: Record<string, number> = {};
    const outdated: Record<string, number> = {};
    for (const i of installs.data ?? []) {
      all[i.skill] = (all[i.skill] ?? 0) + 1;
      if (i.status === "outdated") outdated[i.skill] = (outdated[i.skill] ?? 0) + 1;
    }
    return { installsBySkill: all, outdatedBySkill: outdated };
  }, [installs.data]);

  return { skills, tags, installs, installsBySkill, outdatedBySkill };
}
