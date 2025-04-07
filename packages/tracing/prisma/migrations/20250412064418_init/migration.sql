-- CreateTable
CREATE TABLE "Event" (
    "created_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "build_id" INTEGER NOT NULL,
    "is_event" BOOLEAN NOT NULL DEFAULT true,
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    CONSTRAINT "Event_build_id_fkey" FOREIGN KEY ("build_id") REFERENCES "build" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "hook_call_meta" (
    "plugin_name" TEXT NOT NULL,
    "is_hook_call_meta" BOOLEAN NOT NULL DEFAULT true,
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT
);

-- CreateTable
CREATE TABLE "hook_transform_call" (
    "is_hook_transform_call" BOOLEAN NOT NULL DEFAULT true,
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "event_id" INTEGER NOT NULL,
    "hook_call_meta_id" INTEGER NOT NULL,
    "plugin_hook_transform_start_id" INTEGER NOT NULL,
    "plugin_hook_transform_end_id" INTEGER NOT NULL,
    CONSTRAINT "hook_transform_call_event_id_fkey" FOREIGN KEY ("event_id") REFERENCES "Event" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "hook_transform_call_hook_call_meta_id_fkey" FOREIGN KEY ("hook_call_meta_id") REFERENCES "hook_call_meta" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "hook_transform_call_plugin_hook_transform_start_id_fkey" FOREIGN KEY ("plugin_hook_transform_start_id") REFERENCES "hook_transform_start" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "hook_transform_call_plugin_hook_transform_end_id_fkey" FOREIGN KEY ("plugin_hook_transform_end_id") REFERENCES "hook_transform_end" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "hook_transform_start" (
    "source" TEXT NOT NULL,
    "module_id" TEXT NOT NULL,
    "is_hook_transform_start" BOOLEAN NOT NULL DEFAULT true,
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "event_id" INTEGER NOT NULL,
    "hook_call_meta_id" INTEGER NOT NULL,
    CONSTRAINT "hook_transform_start_event_id_fkey" FOREIGN KEY ("event_id") REFERENCES "Event" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "hook_transform_start_hook_call_meta_id_fkey" FOREIGN KEY ("hook_call_meta_id") REFERENCES "hook_call_meta" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "hook_transform_end" (
    "transformed_source" TEXT NOT NULL,
    "module_id" TEXT NOT NULL,
    "is_plugin_hook_transform_end" BOOLEAN NOT NULL DEFAULT true,
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "event_id" INTEGER NOT NULL,
    "hook_call_meta_id" INTEGER NOT NULL,
    CONSTRAINT "hook_transform_end_event_id_fkey" FOREIGN KEY ("event_id") REFERENCES "Event" ("id") ON DELETE RESTRICT ON UPDATE CASCADE,
    CONSTRAINT "hook_transform_end_hook_call_meta_id_fkey" FOREIGN KEY ("hook_call_meta_id") REFERENCES "hook_call_meta" ("id") ON DELETE RESTRICT ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "build" (
    "created_at" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "is_build" BOOLEAN NOT NULL DEFAULT true,
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT
);

-- CreateIndex
CREATE UNIQUE INDEX "hook_transform_call_event_id_key" ON "hook_transform_call"("event_id");

-- CreateIndex
CREATE UNIQUE INDEX "hook_transform_call_hook_call_meta_id_key" ON "hook_transform_call"("hook_call_meta_id");

-- CreateIndex
CREATE UNIQUE INDEX "hook_transform_call_plugin_hook_transform_start_id_key" ON "hook_transform_call"("plugin_hook_transform_start_id");

-- CreateIndex
CREATE UNIQUE INDEX "hook_transform_call_plugin_hook_transform_end_id_key" ON "hook_transform_call"("plugin_hook_transform_end_id");

-- CreateIndex
CREATE UNIQUE INDEX "hook_transform_start_event_id_key" ON "hook_transform_start"("event_id");

-- CreateIndex
CREATE UNIQUE INDEX "hook_transform_start_hook_call_meta_id_key" ON "hook_transform_start"("hook_call_meta_id");

-- CreateIndex
CREATE UNIQUE INDEX "hook_transform_end_event_id_key" ON "hook_transform_end"("event_id");

-- CreateIndex
CREATE UNIQUE INDEX "hook_transform_end_hook_call_meta_id_key" ON "hook_transform_end"("hook_call_meta_id");
