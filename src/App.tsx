import { BrowserRouter, Routes, Route } from "react-router-dom";
import Sidebar from "./components/Sidebar";
import Subscribe from "./pages/Subscribe";
import MikanBrowser from "./pages/MikanBrowser";
import Download from "./pages/Download";
import Library from "./pages/Library";
import Settings from "./pages/Settings";
import "./styles/theme.css";
import "./styles/App.css";

function App() {
  return (
    <BrowserRouter>
      <div className="app-layout">
        <Sidebar />
        <main className="main-content">
          <Routes>
            <Route path="/" element={<Subscribe />} />
            <Route path="/browse" element={<MikanBrowser />} />
            <Route path="/downloads" element={<Download />} />
            <Route path="/library" element={<Library />} />
            <Route path="/settings" element={<Settings />} />
          </Routes>
        </main>
      </div>
    </BrowserRouter>
  );
}

export default App;
