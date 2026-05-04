import { useEffect } from "react";
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import Home from "./pages/Home";
import Onboarding from "./pages/Onboarding";
import Advanced from "./pages/Advanced";
import Pairing from "./pages/Pairing";

function App() {
  const navigate = useNavigate();
  const location = useLocation();

  useEffect(() => {
    const completed = localStorage.getItem("onboarding_completed");
    if (!completed && location.pathname === "/") {
      navigate("/onboarding", { replace: true });
    }
  }, []);

  return (
    <div className="min-h-screen bg-cream dark:bg-dark-bg transition-colors duration-300">
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/onboarding" element={<Onboarding />} />
        <Route path="/pairing" element={<Pairing />} />
        <Route path="/advanced" element={<Advanced />} />
      </Routes>
    </div>
  );
}

export default App;
