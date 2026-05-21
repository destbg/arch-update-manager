const KNOWN_RELEASE_NOTES: &[(&str, &str)] = &[
    (
        "gnu.org/software/bash",
        "https://tiswww.case.edu/php/chet/bash/NEWS",
    ),
    ("gcc.gnu.org", "https://gcc.gnu.org/releases.html"),
    ("openssl.org", "https://openssl-library.org/news/changelog/"),
    ("gnupg.org", "https://gnupg.org/news.html"),
    ("openssh.com", "https://www.openssh.org/releasenotes.html"),
    (
        "e2fsprogs.sourceforge.net",
        "https://e2fsprogs.sourceforge.net/e2fsprogs-release.html",
    ),
    (
        "tiswww.case.edu/php/chet/readline",
        "https://tiswww.case.edu/php/chet/readline/CHANGES",
    ),
    ("kernel.org", "https://kernel.org/category/releases.html"),
    ("netfilter.org", "https://www.netfilter.org/news.html"),
    ("networkmanager.dev", "https://networkmanager.dev/blog/"),
    ("openvpn.net", "https://github.com/OpenVPN/openvpn/releases"),
    (
        "isc.org/software/bind",
        "https://downloads.isc.org/isc/bind9/",
    ),
    ("isc.org/bind", "https://downloads.isc.org/isc/bind9/"),
    (
        "thekelleys.org.uk/dnsmasq",
        "https://thekelleys.org.uk/dnsmasq/CHANGELOG",
    ),
    ("curl.se", "https://curl.se/changes.html"),
    ("mesa3d.org", "https://docs.mesa3d.org/relnotes.html"),
    (
        "wayland.freedesktop.org",
        "https://wayland.freedesktop.org/releases.html",
    ),
    ("webkitgtk.org", "https://webkitgtk.org/"),
    (
        "poppler.freedesktop.org",
        "https://poppler.freedesktop.org/releases.html",
    ),
    ("freetype.org", "https://freetype.org/"),
    ("cairographics.org", "https://cairographics.org/news/"),
    (
        "imagemagick.org",
        "https://github.com/ImageMagick/ImageMagick/releases",
    ),
    ("icu.unicode.org", "https://icu.unicode.org/download"),
    ("libpng.org", "https://www.libpng.org/pub/png/libpng.html"),
    (
        "openexr.com",
        "https://github.com/AcademySoftwareFoundation/openexr/releases",
    ),
    ("exiv2.org", "https://github.com/Exiv2/exiv2/releases"),
    ("qt.io", "https://www.qt.io/blog/tag/releases"),
    ("mozilla.org/firefox", "https://www.firefox.com/releases/"),
    (
        "thunderbird.net",
        "https://www.thunderbird.net/en-US/thunderbird/releases/",
    ),
    (
        "libreoffice.org",
        "https://www.libreoffice.org/download/release-notes/",
    ),
    ("gimp.org", "https://www.gimp.org/release-notes/"),
    ("videolan.org/vlc", "https://www.videolan.org/vlc/releases/"),
    ("ffmpeg.org", "https://ffmpeg.org/index.html#news"),
    ("vim.org", "https://www.vim.org/news/news.php"),
    (
        "gnu.org/software/emacs",
        "https://www.gnu.org/software/emacs/history.html",
    ),
    (
        "audacityteam.org",
        "https://github.com/audacity/audacity/releases",
    ),
    ("krita.org", "https://krita.org/en/release-history/"),
    (
        "darktable.org",
        "https://github.com/darktable-org/darktable/releases",
    ),
    (
        "community.kde.org/frameworks",
        "https://kde.org/announcements/frameworks/",
    ),
    ("kontact.kde.org", "https://kde.org/announcements/"),
    ("kde.org", "https://kde.org/announcements/"),
    ("mauikit.org", "https://mauikit.org/blog/"),
    (
        "documentfoundation.org",
        "https://www.libreoffice.org/download/release-notes/",
    ),
    ("xorg.freedesktop.org", "https://www.x.org/releases/"),
    (
        "gstreamer.freedesktop.org",
        "https://gstreamer.freedesktop.org/releases/",
    ),
    ("qemu.org", "https://www.qemu.org/blog/"),
    ("mate-desktop.org", "https://mate-desktop.org/blog/"),
    ("erlang.org", "https://www.erlang.org/news"),
    ("opensearch.org", "https://opensearch.org/blog"),
    ("docs.opensearch.org", "https://opensearch.org/blog"),
    (
        "dotnet.microsoft.com",
        "https://github.com/dotnet/core/releases",
    ),
    ("python.org", "https://docs.python.org/3/whatsnew/"),
    ("perl.org", "https://www.perl.org/get.html"),
    ("ruby-lang.org", "https://www.ruby-lang.org/en/news/"),
    ("golang.org", "https://go.dev/doc/devel/release"),
    ("go.dev", "https://go.dev/doc/devel/release"),
    ("rust-lang.org", "https://blog.rust-lang.org/"),
    ("lua.org", "https://www.lua.org/versions.html"),
    ("php.net", "https://www.php.net/ChangeLog-8.php"),
    ("postgresql.org", "https://www.postgresql.org/docs/release/"),
    ("mariadb.org", "https://mariadb.com/kb/en/release-notes/"),
    ("sqlite.org", "https://www.sqlite.org/changes.html"),
    ("valkey.io", "https://github.com/valkey-io/valkey/releases"),
    ("libusb.info", "https://github.com/libusb/libusb/releases"),
    (
        "lib.openmpt.org/libopenmpt",
        "https://lib.openmpt.org/libopenmpt/",
    ),
    ("zlib.net", "https://github.com/madler/zlib/releases"),
    (
        "nodejs.org",
        "https://nodejs.org/en/about/previous-releases",
    ),
    (
        "tukaani.org/xz",
        "https://github.com/tukaani-project/xz/releases",
    ),
    (
        "valgrind.org",
        "https://valgrind.org/downloads/current.html",
    ),
    ("cmake.org", "https://github.com/Kitware/CMake/releases"),
    (
        "ninja-build.org",
        "https://github.com/ninja-build/ninja/releases",
    ),
    (
        "fishshell.com",
        "https://github.com/fish-shell/fish-shell/releases",
    ),
    (
        "zsh.sourceforge.io",
        "https://zsh.sourceforge.io/releases.html",
    ),
    ("zsh.org", "https://zsh.sourceforge.io/releases.html"),
    (
        "greenwoodsoftware.com/less",
        "https://www.greenwoodsoftware.com/less/news.html",
    ),
    (
        "invisible-island.net/ncurses",
        "https://invisible-island.net/ncurses/announce.html",
    ),
    ("pcre.org", "https://www.pcre.org/news.txt"),
    ("nginx.org", "https://nginx.org/en/CHANGES"),
    ("gtkmm.org", "https://gtkmm.gnome.org/en/news.html"),
    (
        "gnu.org/software/binutils",
        "https://sourceware.org/pub/binutils/releases/",
    ),
    (
        "tug.org/texlive",
        "https://svn.tug.org:8369/texlive/trunk/Build/source/texk/web2c/euptexdir/ChangeLog?view=markup",
    ),
    (
        "openjdk.java.net",
        "https://openjdk.org/projects/jdk-updates/",
    ),
    ("openjdk.org", "https://openjdk.org/projects/jdk-updates/"),
    (
        "gambas.sourceforge.net",
        "https://gambaswiki.org/website/en/main.html",
    ),
    (
        "gambaswiki.org",
        "https://gambaswiki.org/website/en/main.html",
    ),
    (
        "pipewire.org",
        "https://gitlab.freedesktop.org/pipewire/pipewire/-/releases",
    ),
    ("hslua.org", "https://github.com/hslua/hslua/releases"),
    (
        "pcp.io",
        "https://github.com/performancecopilot/pcp/releases",
    ),
    ("wiki.gnome.org", "https://release.gnome.org/"),
    ("apps.gnome.org", "https://release.gnome.org/"),
    ("gnome.pages.gitlab.gnome.org", "https://release.gnome.org/"),
    ("gnome.org", "https://release.gnome.org/"),
    ("apps.kde.org", "https://kde.org/announcements/"),
    ("xfce.org", "https://www.xfce.org/about/news"),
    ("docs.xfce.org", "https://www.xfce.org/about/news"),
    (
        "ftp.nluug.nl/pub/vim/runtime/spell",
        "https://www.vim.org/news/news.php",
    ),
    ("docs.gimp.org", "https://www.gimp.org/release-notes/"),
    ("archlinux.org", "https://archlinux.org/news/"),
];

const KNOWN_GITLAB_HOSTS: &[&str] = &["salsa.debian.org", "invent.kde.org", "code.videolan.org"];

pub fn release_notes_url(homepage: &str) -> Option<String> {
    if let Some(url) = forge_release_notes(homepage) {
        return Some(url);
    }
    if let Some(url) = index_site_release_notes(homepage) {
        return Some(url);
    }
    return known_release_notes(homepage);
}

fn forge_release_notes(homepage: &str) -> Option<String> {
    let stripped = homepage
        .strip_prefix("https://")
        .or_else(|| homepage.strip_prefix("http://"))?;

    let (host, path) = stripped.split_once('/')?;
    let host = host.trim_end_matches('.');
    let host = host.strip_prefix("www.").unwrap_or(host);

    let segments: Vec<&str> = path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if segments.len() < 2 {
        return None;
    }

    if host.eq_ignore_ascii_case("github.com") {
        let owner = segments[0];
        let repo = segments[1].trim_end_matches(".git");
        return Some(format!("https://github.com/{}/{}/releases", owner, repo));
    }

    if host.eq_ignore_ascii_case("codeberg.org") {
        let owner = segments[0];
        let repo = segments[1].trim_end_matches(".git");
        return Some(format!("https://codeberg.org/{}/{}/releases", owner, repo));
    }

    if is_gitlab_host(host) {
        let mut project_segments: Vec<&str> = Vec::new();
        for seg in &segments {
            if *seg == "-" {
                break;
            }
            project_segments.push(seg);
        }
        if project_segments.len() < 2 {
            return None;
        }
        if let Some(last) = project_segments.last_mut() {
            *last = last.trim_end_matches(".git");
        }
        return Some(format!(
            "https://{}/{}/-/releases",
            host,
            project_segments.join("/")
        ));
    }

    return None;
}

fn index_site_release_notes(homepage: &str) -> Option<String> {
    let key = normalize_homepage(homepage)?;

    if let Some(rest) = key
        .strip_prefix("metacpan.org/pod/")
        .or_else(|| key.strip_prefix("metacpan.org/dist/"))
        .or_else(|| key.strip_prefix("metacpan.org/release/"))
        .or_else(|| key.strip_prefix("search.cpan.org/dist/"))
        .or_else(|| key.strip_prefix("search.cpan.org/~"))
    {
        let name = rest.split('/').next().unwrap_or("").replace("::", "-");
        if !name.is_empty() {
            return Some(format!("https://metacpan.org/release/{}", name));
        }
    }

    if let Some(rest) = key.strip_prefix("hackage.haskell.org/package/") {
        let segment = rest.split('/').next().unwrap_or("");
        let name = match segment.rsplit_once('-') {
            Some((n, v)) if !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.') => n,
            _ => segment,
        };
        if !name.is_empty() {
            return Some(format!(
                "https://hackage.haskell.org/package/{}/changelog",
                name
            ));
        }
    }

    if let Some(rest) = key
        .strip_prefix("pypi.org/project/")
        .or_else(|| key.strip_prefix("pypi.python.org/pypi/"))
        .or_else(|| key.strip_prefix("pypi.org/pypi/"))
    {
        let name = rest.split('/').next().unwrap_or("");
        if !name.is_empty() {
            return Some(format!("https://pypi.org/project/{}/#history", name));
        }
    }

    return None;
}

fn known_release_notes(homepage: &str) -> Option<String> {
    let key = normalize_homepage(homepage)?;
    for (pattern, target) in KNOWN_RELEASE_NOTES {
        if key.starts_with(pattern) {
            return Some((*target).to_string());
        }
    }
    return None;
}

fn normalize_homepage(homepage: &str) -> Option<String> {
    let stripped = homepage
        .strip_prefix("https://")
        .or_else(|| homepage.strip_prefix("http://"))?;
    let stripped = stripped.strip_prefix("www.").unwrap_or(stripped);
    return Some(stripped.trim_end_matches('/').to_ascii_lowercase());
}

fn is_gitlab_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    if host == "gitlab.com" {
        return true;
    }
    if host.starts_with("gitlab.") || host.contains(".gitlab.") {
        return true;
    }
    return KNOWN_GITLAB_HOSTS.iter().any(|h| host == *h);
}
