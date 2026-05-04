import { useNavigate } from "react-router-dom";

function Onboarding() {
  const navigate = useNavigate();

  return (
    <div className="flex flex-col items-center justify-center min-h-screen px-8 py-12">
      <div className="flex flex-col items-center gap-6 max-w-sm w-full">
        <div className="w-16 h-16 rounded-2xl bg-mint/20 flex items-center justify-center">
          <span className="text-2xl text-mint">+</span>
        </div>

        <h2 className="text-2xl font-bold text-gray-800 dark:text-gray-100">
          Pair a Device
        </h2>

        <p className="text-center text-gray-500 dark:text-gray-400 leading-relaxed">
          Scan the QR code from your other device to start syncing your
          clipboard.
        </p>

        <div className="w-48 h-48 rounded-2xl bg-gray-100 dark:bg-dark-surface flex items-center justify-center mt-4">
          <span className="text-gray-300 dark:text-gray-600 text-sm">
            QR Placeholder
          </span>
        </div>

        <button
          onClick={() => navigate("/")}
          className="mt-4 py-3 px-6 bg-coral text-white rounded-2xl font-medium
                     hover:opacity-90 active:scale-[0.98] transition-all duration-150"
        >
          Done
        </button>
      </div>
    </div>
  );
}

export default Onboarding;
