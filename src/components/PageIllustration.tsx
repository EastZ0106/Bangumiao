import SubscribeIllustration from "../assets/illustrations/Subscribe";
import BrowseIllustration from "../assets/illustrations/Browse";
import DownloadsIllustration from "../assets/illustrations/Downloads";
import LibraryIllustration from "../assets/illustrations/Library";
import SettingsIllustration from "../assets/illustrations/Settings";
import HelpIllustration from "../assets/illustrations/Help";
import AboutIllustration from "../assets/illustrations/About";
import type { ReactNode } from "react";

const illustrations: Record<string, ReactNode> = {
  "/": <SubscribeIllustration />,
  "/browse": <BrowseIllustration />,
  "/downloads": <DownloadsIllustration />,
  "/library": <LibraryIllustration />,
  "/settings": <SettingsIllustration />,
  "/help": <HelpIllustration />,
  "/about": <AboutIllustration />,
};

interface Props {
  page: string;
}

export default function PageIllustration({ page }: Props) {
  const component = illustrations[page];
  return <>{component ?? null}</>;
}
