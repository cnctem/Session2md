import { FileDown } from "lucide-react";
import { useTranslation } from "react-i18next";
import { SessionManagerPage } from "@/components/sessions/SessionManagerPage";

function App() {
  const { t } = useTranslation();

  return (
    <div className="flex h-screen min-h-0 flex-col overflow-hidden bg-background text-foreground">
      <header className="flex h-14 shrink-0 items-center gap-3 border-b px-5">
        <div className="flex size-7 items-center justify-center rounded-md bg-primary text-primary-foreground">
          <FileDown className="size-4" />
        </div>
        <h1 className="text-sm font-semibold">Session2md</h1>
        <span className="text-muted-foreground">/</span>
        <span className="text-sm text-muted-foreground">{t("sessionManager.title")}</span>
      </header>
      <main className="min-h-0 flex-1">
        <SessionManagerPage />
      </main>
    </div>
  );
}

export default App;
