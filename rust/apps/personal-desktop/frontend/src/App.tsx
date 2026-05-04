import { Routes, Route } from "react-router-dom";
import Home from "./pages/Home";
import Onboarding from "./pages/Onboarding";
import Advanced from "./pages/Advanced";

function App() {
  return (
    <div className="min-h-screen bg-cream dark:bg-dark-bg transition-colors duration-300">
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/onboarding" element={<Onboarding />} />
        <Route path="/advanced" element={<Advanced />} />
      </Routes>
    </div>
  );
}

export default App;
