import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import { RangeProvider } from "./context/RangeContext";
import { PageProvider } from "./context/PageContext";
import "./index.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 2,
      refetchOnWindowFocus: true,
      // Per-query refetchInterval is set in useAw hooks where it matters.
    },
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <RangeProvider>
        <PageProvider>
          <App />
        </PageProvider>
      </RangeProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
