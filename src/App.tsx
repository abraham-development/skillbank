import { Route, Routes } from "react-router";
import Sidebar from "./components/Sidebar";
import LibraryPage from "./pages/LibraryPage";
import SkillPage from "./pages/SkillPage";
import AgentsPage from "./pages/AgentsPage";
import { useBankEvents } from "./lib/useBankEvents";

export default function App() {
  useBankEvents();
  return (
    <div className="flex h-full">
      <Sidebar />
      <main className="min-w-0 flex-1">
        <Routes>
          <Route path="/" element={<LibraryPage />} />
          <Route path="/skill/:name" element={<SkillPage />} />
          <Route path="/agentes" element={<AgentsPage />} />
        </Routes>
      </main>
    </div>
  );
}
