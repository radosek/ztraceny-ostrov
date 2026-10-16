// Browser storage (idb). files: game files as Blobs by lower-case name ·
// saves: bytes by "SaveGame/<name>.sav" · kv: "cfg" (Main.cfg bytes).
import { openDB } from "idb";

export const db = openDB("boxb-ztraceny-ostrov", 1, {
  upgrade(d) {
    for (const name of ["files", "saves", "kv"]) {
      if (!d.objectStoreNames.contains(name)) d.createObjectStore(name);
    }
  },
});
