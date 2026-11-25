// Refresca los datos cuando el banco o el registro cambian en disco (p. ej. un agente
// ejecutó `skillbank push` mientras la app está abierta).
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";

export function useBankEvents() {
  const qc = useQueryClient();
  useEffect(() => {
    const unlisteners = [
      listen("bank-changed", () => {
        qc.invalidateQueries({ queryKey: ["bank"] });
        qc.invalidateQueries({ queryKey: ["skill"] });
        qc.invalidateQueries({ queryKey: ["app"] });
      }),
      listen("installs-changed", () => {
        qc.invalidateQueries({ queryKey: ["installs"] });
        qc.invalidateQueries({ queryKey: ["skill"] });
      }),
    ];
    return () => {
      unlisteners.forEach((p) => p.then((un) => un()));
    };
  }, [qc]);
}
