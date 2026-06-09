import { NavLink } from "react-router-dom";
import "./Sidebar.css";

const navItems = [
  { to: "/", label: "订阅列表", icon: "📡" },
  { to: "/browse", label: "蜜柑计划", icon: "🌐" },
  { to: "/downloads", label: "下载管理", icon: "⬇" },
  { to: "/library", label: "本地番剧", icon: "📂" },
  { to: "/settings", label: "设置", icon: "⚙" },
];

export default function Sidebar() {
  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <span className="sidebar-logo">🐱</span>
        <span className="sidebar-title">bangumiao</span>
      </div>
      <nav className="sidebar-nav">
        {navItems.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) =>
              `sidebar-link${isActive ? " active" : ""}`
            }
          >
            <span className="sidebar-icon">{item.icon}</span>
            <span>{item.label}</span>
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
