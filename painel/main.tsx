import { createRoot } from "react-dom/client";
import "./components/arc/foundation.css";
import "./estilo.css";
import { Lugar } from "./lugar";
createRoot(document.getElementById("lugar")!).render(<Lugar />);
