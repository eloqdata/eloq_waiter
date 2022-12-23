## MonographDB Deployment and Management CLI

### Design and implementation

#### Terminology

1. Clusters
   <p>A cluster is a logical concept; a cluster includes a set of MonographDB database instances and supports multiple
   cluster installations. Cluster names must be unique.</p>
2. Command
   <p>User input commands, such as deploy, install, start, stop, etc.</p>
3. TaskExecutor
   <p>A task is an indivisible unit of execution and the smallest parallel unit of execution. A command consists of multiple task instances, and a task instance is a specific instantiated task. For example, CassandraCtlTask represents the task that controls Cassandra, while CassandraCtlTask on Host1 is the specific task instance that indicates CassandraCtlTask will run on node Host1.</p>
4. TaskGroup
   <p>Instances of the same or different types of tasks form a task group, and the tasks in the task group are executed in parallel. Commands and task groups are one-to-one.</p>
5. Parallel mechanisms
```
+ --------paralle-------- + Pause  +  ------parallel----- +

+-----+------+------+------+--------+------+-------+-------+-------+
|     |      |      |      |        |      |       |       |       |
|task1| task2| task3| task4| Barrier|task5 | task6 | task7 |       |
+-----+------+------+------+--------+------+-------+-------+-------+
```
### Command list 