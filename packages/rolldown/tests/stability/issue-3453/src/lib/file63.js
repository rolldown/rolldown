export function Storage(name) {
    this.ready = new Promise((resolve, reject)=>{
        var request = window.indexedDB.open(location.origin);
        request.onupgradeneeded = (e)=>{
            this.db = e.target["result"];
            this.db.createObjectStore("store");
        };
        request.onsuccess = (e)=>{
            this.db = e.target["result"];
            resolve();
        };
        request.onerror = (e)=>{
            this.db = e.target["result"];
            reject(e);
        };
    });
}
Storage.prototype.get = function(key) {
    return this.ready.then(()=>{
        return new Promise((resolve, reject)=>{
            var request = this.getStore().get(key);
            request.onsuccess = (e)=>resolve(e.target.result)
            ;
            request.onerror = reject;
        });
    });
};
Storage.prototype.getStore = function() {
    return this.db.transaction([
        "store"
    ], "readwrite").objectStore("store");
};
Storage.prototype.set = function(key, value) {
    return this.ready.then(()=>{
        return new Promise((resolve, reject)=>{
            var request = this.getStore().put(value, key);
            request.onsuccess = resolve;
            request.onerror = reject;
        });
    });
};
Storage.prototype.delete = function(key, value) {
    window.indexedDB.deleteDatabase(location.origin);
};

export const __FramerMetadata__ = {"exports":{"Storage":{"type":"function"}}}