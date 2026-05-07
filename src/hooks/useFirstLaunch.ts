import { useCallback, useEffect, useState } from "react";
import { wizard } from "@/lib/tracking";

interface UseFirstLaunch {
  isLoading: boolean;
  complete: boolean;
  markComplete: () => Promise<void>;
}

export function useFirstLaunch(): UseFirstLaunch {
  const [isLoading, setIsLoading] = useState(true);
  const [complete, setComplete] = useState(false);

  useEffect(() => {
    let cancelled = false;
    wizard
      .isComplete()
      .then((v) => {
        if (!cancelled) setComplete(v);
      })
      .catch(() => {
        // If the probe fails (config dir unwritable etc.), surface the wizard
        // so the user can try anyway.
        if (!cancelled) setComplete(false);
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const markComplete = useCallback(async () => {
    await wizard.setComplete(true);
    setComplete(true);
  }, []);

  return { isLoading, complete, markComplete };
}
