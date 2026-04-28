PREFIX ?= /usr/local
BINDIR = $(PREFIX)/bin
DATADIR = $(PREFIX)/share

BINARIES = clipsync-server clipsync-client

.PHONY: all install uninstall

all:
	@echo "Run 'make install' to install ClipSync binaries."
	@echo "Run 'make uninstall' to remove them."

install: $(addprefix install-,$(BINARIES))

install-clipsync-%: ../../target/release/clipsync-%
	install -Dm755 $< $(DESTDIR)$(BINDIR)/clipsync-$*
	install -Dm644 clipsync-$*.desktop $(DESTDIR)$(DATADIR)/applications/clipsync-$*.desktop

uninstall:
	$(foreach bin,$(BINARIES),rm -f $(DESTDIR)$(BINDIR)/$(bin);)
	$(foreach bin,$(BINARIES),rm -f $(DESTDIR)$(DATADIR)/applications/$(bin).desktop;)
