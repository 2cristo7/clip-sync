import { useNavigate } from "react-router-dom";
import { toggleTheme } from "../theme";

function Home() {
  const navigate = useNavigate();

  return (
    <div className="flex flex-col items-center justify-center min-h-screen px-8 py-12">
      <div className="flex flex-col items-center gap-6 max-w-sm w-full">
        <h1 className="text-3xl font-bold text-gray-800 dark:text-gray-100">
          ClipSync
        </h1>

        <p className="text-center text-gray-500 dark:text-gray-400 leading-relaxed">
          Your clipboard, everywhere. Seamlessly sync between your devices.
        </p>

        <div className="w-full flex flex-col gap-3 mt-4">
          <button
            onClick={() => navigate("/onboarding")}
            className="w-full py-3 px-6 bg-coral text-white rounded-2xl font-medium
                       hover:opacity-90 active:scale-[0.98] transition-all duration-150"
          >
            Get Started
          </button>
        </div>

        <button
          onClick={toggleTheme}
          className="mt-6 p-2 rounded-xl text-gray-400 dark:text-gray-500
                     hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
          aria-label="Toggle theme"
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
          </svg>
        </button>

        <button
          onClick={() => navigate("/advanced")}
          className="text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600
                     dark:hover:text-gray-300 transition-colors"
        >
          Advanced Settings
        </button>
      </div>
    </div>
  );
}

export default Home;
