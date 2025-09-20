#ifndef LIBLZMA_SYS_LZMA_H
#define LIBLZMA_SYS_LZMA_H

/*
 * This shim selects the correct liblzma header depending on how the crate
 * links against liblzma. When pkg-config succeeds we rely on the system copy,
 * otherwise we fall back to the vendored headers that ship in xz/.
 */
#ifdef PKG_CONFIG
#  include <lzma.h>
#else
#  include "xz/src/liblzma/api/lzma.h"
#endif

#endif /* LIBLZMA_SYS_LZMA_H */
