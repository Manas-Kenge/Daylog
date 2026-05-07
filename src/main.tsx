import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient } from "@tanstack/react-query";
import { PersistQueryClientProvider } from "@tanstack/react-query-persist-client";
import { createSyncStoragePersister } from "@tanstack/query-sync-storage-persister";
import App from "./App";
import { RangeProvider } from "./context/RangeContext";
import { PageProvider } from "./context/PageContext";
import "./index.css";

const APP_VERSION = "0.1.1";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 2,
      refetchOnWindowFocus: true,
      // gcTime needs to outlive a window-close so the persisted cache
      // has fresh-enough entries to rehydrate on the next launch.
      // 24h covers the common "open it again tomorrow morning" case.
      gcTime: 24 * 60 * 60 * 1000,
      // Per-query refetchInterval / staleTime is set in useAw hooks
      // where it matters.
    },
  },
});

const persister = createSyncStoragePersister({
  storage: window.localStorage,
  key: "daylog.rq-cache",
  throttleTime: 1000,
});

/**
 * Buster invalidates the persisted cache when something incompatible
 * changes: app version (query shapes, IPC contracts) or the schema
 * keys we serialize. Keep this in sync with `package.json` version.
 */
const cacheBuster = `daylog@${APP_VERSION}`;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <PersistQueryClientProvider
      client={queryClient}
      persistOptions={{
        persister,
        buster: cacheBuster,
        // Keep entries up to gcTime; the persister's own TTL would
        // otherwise shadow our 24h gcTime with its 24h default.
        maxAge: 24 * 60 * 60 * 1000,
      }}
    >
      <RangeProvider>
        <PageProvider>
          <App />
        </PageProvider>
      </RangeProvider>
    </PersistQueryClientProvider>
  </React.StrictMode>,
);
