import { useNavigate } from "react-router-dom";
import { toggleTheme } from "../theme";

function Advanced() {
  const navigate = useNavigate();

  return (
    <div className="flex flex-col min-h-screen px-8 py-12">
      <div className="flex items-center gap-3 mb-8">
        <button
          onClick={() => navigate("/")}
          className="p-2 rounded-xl text-gray-400 hover:text-gray-600
                     dark:text-gray-500 dark:hover:text-gray-300 transition-colors"
          aria-label="Go back"
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
            <path d="m12 19-7-7 7-7" />
            <path d="M19 12H5" />
          </svg>
        </button>
        <h2 className="text-xl font-bold text-gray-800 dark:text-gray-100">
          Advanced
        </h2>
      </div>

      <div className="flex flex-col gap-4">
        <div className="p-4 rounded-2xl bg-white dark:bg-dark-surface">
          <div className="flex items-center justify-between">
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
              Dark Mode
            </span>
            <button
              onClick={toggleTheme}
              className="px-4 py-1.5 text-xs rounded-xl bg-gray-100 dark:bg-dark-bg
                         text-gray-600 dark:text-gray-400 hover:opacity-80 transition-opacity"
            >
              Toggle
            </button>
          </div>
        </div>

        <div className="p-4 rounded-2xl bg-white dark:bg-dark-surface">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            Network
          </span>
          <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
            mDNS discovery, Tailscale relay, and connection settings will appear
            here.
          </p>
        </div>

        <div className="p-4 rounded-2xl bg-white dark:bg-dark-surface">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            Security
          </span>
          <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
            Token management, pairing secrets, and encryption options will appear
            here.
          </p>
        </div>
      </div>
    </div>
  );
}

export default Advanced;
